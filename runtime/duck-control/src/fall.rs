//! 根据投影重力与角速度预测倾倒趋势。
//!
//! 使用 g_dot = -omega × g 外推重力方向，结合当前倾角和时间去抖
//! 输出倾倒预判。它与静态倒地判断、起身阶段切换分开处理。

use std::time::Duration;

use crate::imu::ImuData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FallPredictorConfig {
    /// Projected-gravity z the robot must *already* be past before a prediction counts.
    /// Upright is -1.0; -0.90 is about 26° of tilt.
    pub tilt_z: f64,
    /// Where the extrapolation has to reach to count as a fall in progress. The same sense
    /// as `SafetyConfig::fall_gravity_z`, and by default the same number: "it will be
    /// what safety calls fallen, in `lookahead`".
    pub predicted_z: f64,
    /// How far ahead to extrapolate.
    pub lookahead: Duration,
    /// How long the verdict must hold before it fires.
    pub debounce: Duration,
}

impl Default for FallPredictorConfig {
    fn default() -> Self {
        Self {
            tilt_z: -0.90,
            predicted_z: -0.5,
            lookahead: Duration::from_millis(300),
            debounce: Duration::from_millis(60),
        }
    }
}

/// Watches the IMU for a fall that has started but not landed.
///
/// Edge-triggered: [`Self::observe`] returns `true` on the tick the debounce completes and
/// then stays `false` until the robot stops looking like it is falling. The caller owns
/// what to do about it and how long to keep doing it — this only reports the moment.
#[derive(Debug, Clone)]
pub struct FallPredictor {
    config: FallPredictorConfig,
    /// How long the three conditions have held together. Any tick that fails one resets it.
    falling_for: Duration,
    /// Whether the trigger has already been handed out for this fall, so a caller polling
    /// every tick gets one edge rather than a stream of them.
    fired: bool,
}

impl FallPredictor {
    pub fn new(config: FallPredictorConfig) -> Self {
        Self {
            config,
            falling_for: Duration::ZERO,
            fired: false,
        }
    }

    /// Rate of change of projected-gravity z, per second, from `ġ = −ω × g`.
    ///
    /// Positive means the trunk is tipping *away* from upright — gravity z climbing from
    /// -1 towards 0. Public because it is the number to look at when tuning the
    /// thresholds against a recording, and it is not otherwise recoverable from outside.
    pub fn gravity_z_rate(imu: &ImuData) -> f64 {
        -(imu.gyro[0] * imu.gravity[1] - imu.gyro[1] * imu.gravity[0])
    }

    /// Where gravity z is heading, `lookahead` from now, at the current rate.
    pub fn predicted_z(&self, imu: &ImuData) -> f64 {
        imu.gravity[2] + Self::gravity_z_rate(imu) * self.config.lookahead.as_secs_f64()
    }

    /// One tick. `true` exactly once per fall, when the debounce completes.
    pub fn observe(&mut self, imu: &ImuData, dt: Duration) -> bool {
        let rate = Self::gravity_z_rate(imu);
        let going_down = imu.gravity[2] > self.config.tilt_z
            && rate > 0.0
            && self.predicted_z(imu) > self.config.predicted_z;

        if !going_down {
            self.falling_for = Duration::ZERO;
            // Re-arm only once the robot stops looking like it is falling, so the caller
            // that limped is not handed a second trigger while the first one is still
            // playing out on the way down.
            self.fired = false;
            return false;
        }

        self.falling_for = self.falling_for.saturating_add(dt);
        if self.falling_for >= self.config.debounce && !self.fired {
            self.fired = true;
            return true;
        }
        false
    }

    /// Forget the accumulated verdict — the caller has taken charge (it is limping, or the
    /// policy stopped driving), so the next fall must be detected from scratch.
    pub fn reset(&mut self) {
        self.falling_for = Duration::ZERO;
        self.fired = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: Duration = Duration::from_millis(20);

    /// Gravity for a trunk pitched `tilt` radians forward, and a pitch rate about y.
    fn tipping(tilt: f64, pitch_rate: f64) -> ImuData {
        ImuData {
            gyro: [0.0, pitch_rate, 0.0],
            gravity: [tilt.sin(), 0.0, -tilt.cos()],
            ..ImuData::default()
        }
    }

    /// The derivative has to be the real one, or every threshold above it is guesswork.
    /// Pitching forward at 1 rad/s from 30° must raise gravity z at sin(30°) = 0.5 per
    /// second — the analytic value, not something close to it.
    #[test]
    fn the_rate_is_the_analytic_derivative() {
        let imu = tipping(std::f64::consts::FRAC_PI_6, 1.0);
        assert!((FallPredictor::gravity_z_rate(&imu) - 0.5).abs() < 1e-12);
    }

    /// Tipping the other way reads negative — recovering from a lean must never look like
    /// a fall, whatever the magnitude of the rate.
    #[test]
    fn recovering_from_a_lean_never_fires() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        // Well past the tilt gate, rotating back towards upright, fast.
        for _ in 0..50 {
            assert!(!p.observe(&tipping(0.9, -4.0), DT));
        }
    }

    /// An upright robot taking a hard footfall: a big gyro transient with no tilt behind
    /// it. This is the false positive that matters — it happens every step.
    #[test]
    fn a_footfall_on_an_upright_robot_never_fires() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        for _ in 0..10 {
            // 8° of lean, 3 rad/s of impulse. The tilt gate is what refuses this.
            assert!(!p.observe(&tipping(0.14, 3.0), DT));
        }
    }

    /// A real fall: tilted past the gate and rotating over at a rate that puts it on the
    /// floor. It must fire, and it must fire *once*.
    #[test]
    fn a_fall_fires_once_after_the_debounce() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        let falling = tipping(0.6, 3.0);
        // 34° of tilt clears the -0.90 gate; the extrapolation clears -0.5.
        assert!(falling.gravity[2] > -0.90);
        assert!(p.predicted_z(&falling) > -0.5);

        // Debounce is 60 ms — three ticks. The first two must stay quiet.
        assert!(!p.observe(&falling, DT));
        assert!(!p.observe(&falling, DT));
        assert!(p.observe(&falling, DT), "the debounce completes on tick 3");
        for _ in 0..20 {
            assert!(!p.observe(&falling, DT), "one edge per fall, not a stream");
        }
    }

    /// A tilt that is not going anywhere — the robot held at an angle by hand — is not a
    /// fall however far it is tipped. Position alone must not trigger this; that verdict
    /// belongs to `Safety`, on its own thresholds.
    #[test]
    fn a_static_tilt_is_not_a_fall() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        for _ in 0..50 {
            assert!(!p.observe(&tipping(1.2, 0.0), DT));
        }
    }

    /// A slow topple that has not committed yet — tilted, tipping, but slowly enough that
    /// the standing policy has a quarter-second to catch it. Waiting is the right answer:
    /// limping here would *cause* the fall it is predicting.
    #[test]
    fn a_slow_lean_waits() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        // 26° and 0.3 rad/s: predicted z is about -0.86, nowhere near the -0.5 floor.
        let leaning = tipping(0.46, 0.3);
        assert!(leaning.gravity[2] > -0.90, "past the tilt gate");
        for _ in 0..50 {
            assert!(!p.observe(&leaning, DT));
        }
    }

    /// The trigger re-arms only after the robot stops looking like it is falling, so a
    /// second fall is caught but the tail of the first one is not re-reported.
    #[test]
    fn it_rearms_when_the_fall_stops() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        let falling = tipping(0.6, 3.0);
        for _ in 0..3 {
            p.observe(&falling, DT);
        }
        assert!(!p.observe(&falling, DT), "already fired");
        // Upright and still: the verdict clears.
        assert!(!p.observe(&tipping(0.0, 0.0), DT));
        // And a fresh fall fires again, after its own debounce.
        assert!(!p.observe(&falling, DT));
        assert!(!p.observe(&falling, DT));
        assert!(p.observe(&falling, DT));
    }

    /// `reset` drops a debounce in progress: the caller took charge, so the next fall is
    /// detected from scratch rather than completing a count from before.
    #[test]
    fn reset_drops_a_debounce_in_progress() {
        let mut p = FallPredictor::new(FallPredictorConfig::default());
        let falling = tipping(0.6, 3.0);
        p.observe(&falling, DT);
        p.observe(&falling, DT);
        p.reset();
        assert!(!p.observe(&falling, DT), "the count restarted");
        assert!(!p.observe(&falling, DT));
        assert!(p.observe(&falling, DT));
    }
}
