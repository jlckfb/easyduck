//! 关节目标范围、堵转和温度保护。
//!
//! 根据关节反馈与温度，限制目标、暂时保持受阻关节或降低增益。
//! 本层不直接访问总线，也不定义 HD1910 固件报警寄存器。

use std::time::Duration;

use crate::io::Sensors;
use crate::model::{JOINT_IDS, JOINT_NAMES, NUM_JOINTS, clamp_joint_targets};

/// How long a joint is held off after a stall before it is driven again.
///
/// One second at 50 Hz: long enough that a joint pinned against a stop spends most of its
/// time unloaded — a third of the duty cycle it had — and short enough that a leg blocked by
/// an obstacle gets its motion back within a step or two of the obstacle clearing.
pub const STALL_RETRY: Duration = Duration::from_millis(1000);

/// Hysteresis below `overtemp_c` at which a joint counts as cool again.
///
/// Temperature is a slow, one-degree-quantised signal. Without a gap, a joint sitting on the
/// threshold toggles its softening every time the reading crosses it, and a gain that flips
/// is a joint that twitches.
pub const OVERTEMP_CLEAR_MARGIN_C: f64 = 10.0;

/// How long a joint must stay quiet before its next journal line. Five seconds at 50 Hz.
///
/// The journal is the only place the *joint* is named — the wire carries the reason, not the
/// joint — so a permanently blocked joint must not be able to flood it: without this, a stall
/// re-entered every 1.5 s writes 40 identical lines a minute for as long as it lasts.
pub const WARN_COOLDOWN: Duration = Duration::from_millis(5000);

/// The thresholds, resolved from `robotd-params`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProtectionConfig {
    /// Apply [`crate::model::JOINT_SAFE_RANGE`] to every target. The rollback switch.
    pub limits: bool,
    /// Detect a joint that is pushing without moving, and hold it where it is.
    pub stall: bool,
    /// Soften a joint that is over its thermal threshold.
    pub thermal: bool,
    /// Standing error that counts as *pushing*, radians.
    pub stall_error_rad: f64,
    /// Speed under which a joint counts as *not moving*, rad/s.
    pub stall_speed_rad_s: f64,
    /// Present current that counts as *loaded*, mA.
    pub stall_current_ma: f64,
    /// `|present pwm|` that counts as *loaded*, in the servo's own counts (885 = full).
    pub stall_pwm: f64,
    /// How long all three must hold before the joint is held.
    pub stall_for: Duration,
    /// How long it is held before the next attempt.
    pub stall_retry: Duration,
    /// Case temperature at which a joint is softened, °C.
    pub overtemp_c: f64,
    /// The position P gain a hot joint runs at.
    pub overtemp_gain: u16,
}

impl Default for ProtectionConfig {
    /// The defaults, and why each number is what it is. Structurally the same guards as
    /// `robotd-params`' `[safety]`, which is where a robot overrides them.
    fn default() -> Self {
        Self {
            limits: true,
            stall: true,
            thermal: true,
            // 12°: an error smaller than this is tracking lag, not a blocked joint, and the
            // servo is not being asked to hold a position it cannot reach. The neck incident
            // was 69°.
            stall_error_rad: 12.0_f64.to_radians(),
            // 0.3 rad/s ≈ 17°/s. A walking joint moves at 100°/s and a stalled one at zero;
            // this sits far from both. It is the term that keeps a leg which is merely
            // *lagging* out of the detector — a lagging joint still has velocity.
            stall_speed_rad_s: 0.3,
            // 900 mA is 61% of the XL330-M288's 5 V stall current (1.47 A) and 51% of the
            // servo's own Current Limit register (1750 mA). A joint holding that for half a
            // second with a 12° error and no motion is not working, it is stuck.
            stall_current_ma: 900.0,
            // 800 of the 885 the PWM Limit register allows (90%) — the servo's own output
            // saturated. Kept as a second, independent signal because the current *sensor*
            // is the part of a clone servo least likely to be calibrated, and a detector
            // that can see only one of the two is a detector that silently never fires.
            stall_pwm: 800.0,
            // 25 ticks at 50 Hz. Long enough that no ordinary transient — a footfall, a
            // direction reversal, a landing — reaches it; short enough that a servo pinned
            // at 1.4 A is caught before its case has warmed appreciably.
            stall_for: Duration::from_millis(500),
            stall_retry: STALL_RETRY,
            // 60 °C. The XL330's operating range is −5..+70 °C, its Temperature Limit
            // register defaults to 70 °C, and at 70 the Overheating Error bit sets — and
            // because this robot pins Shutdown(63) with that bit in it, the servo turns its
            // own torque off and needs a power cycle. 60 leaves 10 °C for the core to run
            // hotter than the case it reports, and sits well above the idle case (~32 °C).
            overtemp_c: 60.0,
            // Half the running gain (200). Low enough to take most of the standing current
            // out of a hot joint, high enough that the joint is still holding the robot up:
            // a degradation, not the limp gain.
            overtemp_gain: 100,
        }
    }
}

/// What the guard did to this tick's targets.
///
/// Bitmasks rather than lists: this runs on the control thread every tick, and a `Vec` here
/// would be an allocation per tick on the one thread that must not visit the allocator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Guard {
    /// Joints whose target was moved into their safe travel.
    pub clamped: u16,
    /// Joints commanded where they already are, because they are stuck — or resting from a
    /// stall.
    pub held: u16,
    /// Joints over their thermal threshold, to be run at `overtemp_gain`.
    pub hot: u16,
}

const _: () = assert!(NUM_JOINTS <= u16::BITS as usize);

impl Guard {
    pub fn is_clamped(&self, joint: usize) -> bool {
        self.clamped & (1 << joint) != 0
    }

    pub fn is_held(&self, joint: usize) -> bool {
        self.held & (1 << joint) != 0
    }

    pub fn is_hot(&self, joint: usize) -> bool {
        self.hot & (1 << joint) != 0
    }

    /// The joints in a mask, by name — for a log line or a test assertion. Not for the tick:
    /// this allocates.
    pub fn names(mask: u16) -> Vec<&'static str> {
        (0..NUM_JOINTS)
            .filter(|joint| mask & (1 << joint) != 0)
            .map(|joint| JOINT_NAMES[joint])
            .collect()
    }
}

/// The per-joint state. Owned by [`crate::safety::Safety`], which is what makes the guard
/// unskippable: targets reach a servo only through `Safety`, and `Safety` runs this first.
pub struct Protection {
    target_ranges: Option<[(f64, f64); NUM_JOINTS]>,
    config: ProtectionConfig,
    /// The last sample. A stall is measured against these positions, so with no sample the
    /// guard has nothing to say — and says nothing.
    present: [f64; NUM_JOINTS],
    currents_ma: [f64; NUM_JOINTS],
    pwms: [f64; NUM_JOINTS],
    speeds: [f64; NUM_JOINTS],
    /// Case temperatures, from [`Self::observe_thermals`]. Zero means "no reading", never
    /// "cold": a bus that has stopped answering must not talk a hot joint down.
    temps_c: [f64; NUM_JOINTS],
    /// How long each joint has been stuck: pushing, loaded, and not moving.
    stuck_for: [Duration; NUM_JOINTS],
    /// How long each joint is held off for. Non-zero means "commanded where it is".
    retry_in: [Duration; NUM_JOINTS],
    /// Thermal verdicts, hysteretic. See [`OVERTEMP_CLEAR_MARGIN_C`].
    hot: [bool; NUM_JOINTS],
    /// Rate limit on the journal, per joint. Counts down.
    quiet_for: [Duration; NUM_JOINTS],
    /// Whether this joint's current stall was reported. What the release line keys off, so
    /// the journal tells a whole story for the episodes it reports and stays quiet about the
    /// ones it rate-limited away — otherwise the "driving it again" line, arriving half a
    /// second earlier in each cycle, takes the budget and the stall it belongs to never gets
    /// a line at all.
    announced: [bool; NUM_JOINTS],
    /// The tick's length, from the last [`Self::observe`].
    tick: Duration,
    /// Stalls entered, per joint, since startup — and journal lines actually written for
    /// them. Two counters, because "how often is this joint stuck" and "how often did
    /// anybody hear about it" are different questions and the second is the rate limiter's.
    pub stalls: [u64; NUM_JOINTS],
    /// Journal lines written for stalls, per joint.
    pub stall_warnings: [u64; NUM_JOINTS],
    /// Overheats entered, per joint, since startup.
    pub overheats: [u64; NUM_JOINTS],
}

impl Protection {
    pub fn new(config: ProtectionConfig) -> Self {
        Self {
            config,
            target_ranges: None,
            present: [0.0; NUM_JOINTS],
            currents_ma: [0.0; NUM_JOINTS],
            pwms: [0.0; NUM_JOINTS],
            speeds: [0.0; NUM_JOINTS],
            temps_c: [0.0; NUM_JOINTS],
            stuck_for: [Duration::ZERO; NUM_JOINTS],
            retry_in: [Duration::ZERO; NUM_JOINTS],
            hot: [false; NUM_JOINTS],
            quiet_for: [Duration::ZERO; NUM_JOINTS],
            announced: [false; NUM_JOINTS],
            tick: Duration::ZERO,
            stalls: [0; NUM_JOINTS],
            stall_warnings: [0; NUM_JOINTS],
            overheats: [0; NUM_JOINTS],
        }
    }

    pub fn set_target_ranges(&mut self, ranges: Option<[(f64, f64); NUM_JOINTS]>) {
        self.target_ranges = ranges;
    }

    pub fn config(&self) -> &ProtectionConfig {
        &self.config
    }

    /// Is this joint resting from a stall right now?
    pub fn is_resting(&self, joint: usize) -> bool {
        self.retry_in[joint] > Duration::ZERO
    }

    /// Is this joint over its thermal threshold right now?
    pub fn is_hot(&self, joint: usize) -> bool {
        self.hot[joint]
    }

    /// Record a fresh sample. Call once per tick, before [`Self::guard`].
    ///
    /// Repeating a sample for a coasted tick is *not* wanted: the timers advance on the
    /// clock, and a joint that has stopped being read has stopped being watched.
    pub fn observe(&mut self, sensors: &Sensors, dt: Duration) {
        self.present = sensors.positions;
        self.currents_ma = sensors.currents_ma;
        self.pwms = sensors.pwms;
        self.speeds = sensors.velocities;
        self.tick = dt;
    }

    /// Record the temperatures from the slow read — see [`crate::io::RobotIo::slow_sensors`],
    /// which is where they arrive. Once a second is the sampling rate a case temperature
    /// deserves.
    pub fn observe_thermals(&mut self, temps_c: &[f64; NUM_JOINTS]) {
        self.temps_c = *temps_c;
    }

    /// The last thing to touch a target before it is written.
    ///
    /// Clamps every joint into its safe travel, holds any joint that is stuck (or still
    /// resting from a stall), and decides which joints are hot. `targets` is modified in
    /// place; the returned [`Guard`] says what was done — for reporting, and for the per-joint
    /// gain the caller still has to write.
    pub fn guard(&mut self, targets: &mut [f64; NUM_JOINTS]) -> Guard {
        let mut guard = Guard::default();

        for joint in 0..NUM_JOINTS {
            self.quiet_for[joint] = self.quiet_for[joint].saturating_sub(self.tick);
        }

        if self.config.limits {
            if let Some(ranges) = self.target_ranges {
                for (joint, value) in targets.iter_mut().enumerate() {
                    let (lo, hi) = ranges[joint];
                    let clipped = value.clamp(lo, hi);
                    if clipped != *value { guard.clamped |= 1 << joint; }
                    *value = clipped;
                }
            } else {
                guard.clamped = clamp_joint_targets(targets);
            }
        }

        if !self.config.thermal {
            self.hot = [false; NUM_JOINTS];
        }

        // Heat first, so a joint that is both stalled and hot is reported as both, and
        // softened on the same tick it is held.
        if self.config.thermal {
            for joint in 0..NUM_JOINTS {
                let celsius = self.temps_c[joint];
                if self.hot[joint] {
                    // `celsius <= 0.0` is "no reading" and leaves the verdict alone: the way
                    // out of a thermal latch is a cool reading, not a missing one.
                    if celsius > 0.0 && celsius <= self.config.overtemp_c - OVERTEMP_CLEAR_MARGIN_C
                    {
                        self.hot[joint] = false;
                        if self.may_warn(joint) {
                            tracing::info!(
                                joint = JOINT_NAMES[joint],
                                temp_c = celsius,
                                "joint cooled — gain restored"
                            );
                        }
                    }
                } else if celsius >= self.config.overtemp_c {
                    self.hot[joint] = true;
                    self.overheats[joint] = self.overheats[joint].saturating_add(1);
                    if self.may_warn(joint) {
                        tracing::warn!(
                            joint = JOINT_NAMES[joint],
                            id = JOINT_IDS[joint],
                            temp_c = celsius,
                            gain = self.config.overtemp_gain,
                            count = self.overheats[joint],
                            "joint over temperature — softening its gain"
                        );
                    }
                }
                if self.hot[joint] {
                    guard.hot |= 1 << joint;
                }
            }
        }

        for joint in 0..NUM_JOINTS {
            // Resting from a stall: command where the joint is, and count the pause down.
            if self.retry_in[joint] > Duration::ZERO {
                self.retry_in[joint] = self.retry_in[joint].saturating_sub(self.tick);
                if self.retry_in[joint] > Duration::ZERO {
                    targets[joint] = self.present[joint];
                    guard.held |= 1 << joint;
                    continue;
                }
                // The pause ended *on this tick*, so the caller gets its target back now
                // rather than one tick later — and the stall test below starts from zero,
                // which is what makes the duty cycle the thresholds say it is.
                self.stuck_for[joint] = Duration::ZERO;
                if std::mem::take(&mut self.announced[joint]) {
                    tracing::info!(
                        joint = JOINT_NAMES[joint],
                        "joint released — driving it again"
                    );
                }
            }

            if !self.config.stall || !self.is_stuck(joint, targets[joint]) {
                self.stuck_for[joint] = Duration::ZERO;
                continue;
            }

            self.stuck_for[joint] = self.stuck_for[joint].saturating_add(self.tick);
            if self.stuck_for[joint] < self.config.stall_for {
                continue;
            }

            // The error has to be read before the target is replaced, or the log line
            // reports the error of the substitution rather than the one that caused it.
            let error_deg = (targets[joint] - self.present[joint]).abs().to_degrees();
            self.retry_in[joint] = self.config.stall_retry;
            self.stuck_for[joint] = Duration::ZERO;
            self.stalls[joint] = self.stalls[joint].saturating_add(1);
            targets[joint] = self.present[joint];
            guard.held |= 1 << joint;
            if self.may_warn(joint) {
                self.stall_warnings[joint] = self.stall_warnings[joint].saturating_add(1);
                self.announced[joint] = true;
                tracing::warn!(
                    joint = JOINT_NAMES[joint],
                    id = JOINT_IDS[joint],
                    error_deg = format!("{error_deg:.0}"),
                    current_ma = format!("{:.0}", self.currents_ma[joint]),
                    pwm = format!("{:.0}", self.pwms[joint]),
                    stall_ms = self.config.stall_for.as_millis(),
                    count = self.stalls[joint],
                    "joint stalled — holding it where it is"
                );
            }
        }

        guard
    }

    /// A joint being asked to move, loaded, and not moving.
    ///
    /// All three, and each one is doing work:
    ///
    ///  - the **error** says the servo is being asked for a position it is not at — with no
    ///    error there is no torque, and nothing to protect;
    ///  - the **speed** says it is not on its way there. This is the term that separates
    ///    "stuck" from "lagging": a joint that is behind but moving will arrive, and holding
    ///    it would be the guard breaking the robot to save a servo;
    ///  - the **load** says the servo is actually working — current, or the PWM output that
    ///    produces it. Either is enough, because they fail independently.
    fn is_stuck(&self, joint: usize, target: f64) -> bool {
        let error = (target - self.present[joint]).abs();
        let loaded = self.currents_ma[joint] >= self.config.stall_current_ma
            || self.pwms[joint].abs() >= self.config.stall_pwm;
        let still = self.speeds[joint].abs() <= self.config.stall_speed_rad_s;
        error > self.config.stall_error_rad && loaded && still
    }

    /// Rate limit, per joint. True when this joint may write a line — and *consumes* the
    /// permission, so a caller must use the answer.
    fn may_warn(&mut self, joint: usize) -> bool {
        if self.quiet_for[joint] > Duration::ZERO {
            return false;
        }
        self.quiet_for[joint] = WARN_COOLDOWN;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_POSITION, joint_index};

    const TICK: Duration = Duration::from_millis(20);

    /// A robot at home, tracking perfectly: the sample a healthy tick produces.
    fn settled() -> Sensors {
        Sensors {
            positions: DEFAULT_POSITION,
            ..Sensors::default()
        }
    }

    /// A joint pinned against something: not moving, drawing stall current, output saturated.
    fn blocked_at(joint: usize, at: f64, current_ma: f64) -> Sensors {
        let mut sensors = settled();
        sensors.positions[joint] = at;
        sensors.currents_ma[joint] = current_ma;
        sensors.pwms[joint] = 880.0;
        sensors.velocities[joint] = 0.0;
        sensors
    }

    /// A joint somewhere, loaded, moving at `speed`.
    fn loaded(joint: usize, at: f64, current_ma: f64, speed: f64) -> Sensors {
        let mut sensors = settled();
        sensors.positions[joint] = at;
        sensors.currents_ma[joint] = current_ma;
        sensors.pwms[joint] = 300.0;
        sensors.velocities[joint] = speed;
        sensors
    }

    /// One tick through the guard, as `Safety` drives it.
    fn tick(
        protection: &mut Protection,
        sensors: &Sensors,
        targets: &mut [f64; NUM_JOINTS],
    ) -> Guard {
        protection.observe(sensors, TICK);
        protection.guard(targets)
    }

    fn protection_without_the_travel_clamp() -> Protection {
        Protection::new(ProtectionConfig {
            limits: false,
            ..ProtectionConfig::default()
        })
    }

    /// **The regression.** A joint that cannot reach its target, loaded and motionless, is
    /// stopped after half a second — and the counter is what says so.
    #[test]
    fn a_joint_pinned_against_a_stop_is_held_after_half_a_second() {
        let neck = joint_index("neck_pitch").unwrap();
        // The travel clamp off, so this is the stall guard's own work: the guard has to hold
        // a joint the table already believes is reachable.
        let mut protection = protection_without_the_travel_clamp();

        let mut wanted = DEFAULT_POSITION;
        // Well outside the joint's window — a target this servo cannot reach. (It used to be
        // this robot's home pose for the neck; A-6 re-measured that on 2026-09-12 and the
        // point here never depended on it. The guard answers *any* unreachable target.)
        wanted[neck] = -2.05;
        let sensors = blocked_at(neck, -0.8378, 1400.0);

        // Not yet: 24 ticks is under the 25-tick debounce, and the caller's target stands.
        for _ in 0..24 {
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(!outcome.is_held(neck), "held before the debounce elapsed");
            assert_eq!(targets[neck], -2.05);
        }

        let mut targets = wanted;
        let outcome = tick(&mut protection, &sensors, &mut targets);
        assert!(outcome.is_held(neck), "a pinned joint must be held");
        assert_eq!(
            targets[neck], -0.8378,
            "it is commanded where it is, so the error — and the current — collapses"
        );
        assert_eq!(protection.stalls[neck], 1);
        assert_eq!(
            protection.stall_warnings[neck], 1,
            "and it was named in the journal"
        );

        // It stays held for the retry window rather than resuming next tick.
        for _ in 0..49 {
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(outcome.is_held(neck));
            assert_eq!(targets[neck], -0.8378);
        }
        assert_eq!(protection.stalls[neck], 1, "one stall, not fifty");

        // The 50th tick ends the window and the caller's target is tried again.
        let mut targets = wanted;
        let outcome = tick(&mut protection, &sensors, &mut targets);
        assert!(!outcome.is_held(neck), "the retry window must end");
        assert_eq!(targets[neck], -2.05, "and the caller gets its target back");
    }

    /// A joint that is *blocked for good* keeps its duty cycle: half a second of pushing,
    /// then a second of rest. That ratio is the whole point — the average current is what
    /// heats the case, and it falls by two thirds.
    #[test]
    fn a_permanently_stuck_joint_is_driven_a_third_of_the_time() {
        let neck = joint_index("neck_pitch").unwrap();
        let mut protection = protection_without_the_travel_clamp();

        let mut wanted = DEFAULT_POSITION;
        wanted[neck] = -2.05;
        let sensors = blocked_at(neck, -0.8378, 1400.0);

        let mut pushing = 0;
        const TICKS: u32 = 75 * 10;
        for _ in 0..TICKS {
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            if !outcome.is_held(neck) {
                pushing += 1;
            }
        }

        assert_eq!(protection.stalls[neck], 10, "one stall per retry cycle");
        let duty = f64::from(pushing) / f64::from(TICKS);
        assert!(
            (duty - 25.0 / 75.0).abs() < 0.02,
            "duty cycle {duty:.2} should be 25 pushing ticks in every 75"
        );
    }

    /// The detector must not fire on a joint that is merely behind: a walking leg lags its
    /// target constantly, and holding a leg mid-stride is how a guard causes a fall.
    #[test]
    fn a_lagging_joint_that_is_moving_is_left_alone() {
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());

        let mut wanted = DEFAULT_POSITION;
        wanted[knee] = DEFAULT_POSITION[knee] + 0.5; // 28°, well past the error threshold
        // Same error and same current as the stall case — but moving, which is the whole
        // difference between lagging and stuck.
        let sensors = loaded(knee, DEFAULT_POSITION[knee], 1400.0, 2.0);

        for _ in 0..200 {
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(!outcome.is_held(knee), "a moving joint was held as stalled");
            assert_eq!(targets[knee], wanted[knee]);
        }
        assert_eq!(protection.stalls[knee], 0);
    }

    /// Nor on a joint that is loaded and on target — standing still is not a stall, it is
    /// standing. A guard that fires here would disable every joint holding the robot up.
    #[test]
    fn a_loaded_joint_on_target_is_not_a_stall() {
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());
        let sensors = loaded(knee, DEFAULT_POSITION[knee], 1400.0, 0.0);

        for _ in 0..200 {
            let mut targets = DEFAULT_POSITION;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(!outcome.is_held(knee));
            assert_eq!(targets[knee], DEFAULT_POSITION[knee]);
        }
        assert_eq!(protection.stalls[knee], 0);
    }

    /// Nor on a joint that is off target, still, and *unloaded* — being held by hand, or
    /// limp. Nothing is heating, so there is nothing to protect.
    #[test]
    fn an_unloaded_joint_that_is_not_moving_is_not_a_stall() {
        let hip = joint_index("left_hip_pitch").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());

        let mut wanted = DEFAULT_POSITION;
        wanted[hip] = DEFAULT_POSITION[hip] + 0.5;
        // 17 mA is the datasheet's standby current, and 0 pwm is a servo doing nothing.
        let mut sensors = loaded(hip, DEFAULT_POSITION[hip], 17.0, 0.0);
        sensors.pwms[hip] = 0.0;

        for _ in 0..200 {
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(!outcome.is_held(hip));
        }
        assert_eq!(protection.stalls[hip], 0);
    }

    /// PWM alone is enough. The current *sensor* is the least reliable part of a clone
    /// servo, and a detector whose only input is that sensor is one that never fires on the
    /// board that needs it most.
    #[test]
    fn a_saturated_pwm_counts_as_loaded_without_a_current_reading() {
        let neck = joint_index("neck_pitch").unwrap();
        let mut protection = protection_without_the_travel_clamp();

        let mut wanted = DEFAULT_POSITION;
        wanted[neck] = -2.05;
        // No current reported at all — the clone's sensor reads zero under load.
        let sensors = blocked_at(neck, -0.8378, 0.0);

        // 24 ticks is one short of the 25-tick debounce.
        for _ in 0..24 {
            let mut targets = wanted;
            tick(&mut protection, &sensors, &mut targets);
        }
        assert_eq!(protection.stalls[neck], 0);

        let mut targets = wanted;
        let outcome = tick(&mut protection, &sensors, &mut targets);
        assert!(
            outcome.is_held(neck),
            "pwm was the only load signal and it worked"
        );
    }

    /// A single burst of current at a direction reversal must not trip it: the *duration* is
    /// as load-bearing as the thresholds.
    #[test]
    fn a_brief_spike_does_not_stall() {
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());

        let mut wanted = DEFAULT_POSITION;
        wanted[knee] = DEFAULT_POSITION[knee] + 0.5;

        for tick_no in 0..200 {
            // Loaded, still, off target — for three ticks out of every ten.
            let in_spike = tick_no % 10 < 3;
            let sensors = if in_spike {
                blocked_at(knee, DEFAULT_POSITION[knee], 1400.0)
            } else {
                loaded(knee, DEFAULT_POSITION[knee], 200.0, 2.0)
            };
            let mut targets = wanted;
            let outcome = tick(&mut protection, &sensors, &mut targets);
            assert!(
                !outcome.is_held(knee),
                "a three-tick spike tripped the guard"
            );
        }
        assert_eq!(protection.stalls[knee], 0);
    }

    /// The travel clamp runs before anything else, so a wild target is already inside the
    /// mechanism by the time the stall detector looks at it.
    #[test]
    fn the_travel_clamp_bounds_a_wild_target() {
        let neck = joint_index("neck_pitch").unwrap();
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());

        let mut targets = DEFAULT_POSITION;
        targets[neck] = -3.0;
        targets[knee] = 40.0;

        let outcome = tick(&mut protection, &settled(), &mut targets);

        assert!(outcome.is_clamped(neck));
        assert_eq!(targets[neck], crate::model::JOINT_SAFE_RANGE[neck].0);
        assert_eq!(targets[knee], crate::model::JOINT_SAFE_RANGE[knee].1);
        // By joint index, which is the order the mask is built in.
        assert_eq!(
            Guard::names(outcome.clamped),
            vec!["left_knee", "neck_pitch"]
        );
    }

    /// The rollback story: with the switch off, a target past the mechanism is written
    /// exactly as the caller asked. Needed for A/B on a bench — "is this guard the thing
    /// that changed the walk?" has to be answerable.
    #[test]
    fn the_travel_clamp_can_be_switched_off() {
        let neck = joint_index("neck_pitch").unwrap();
        let mut protection = protection_without_the_travel_clamp();

        let mut targets = DEFAULT_POSITION;
        targets[neck] = -3.0;
        let outcome = tick(&mut protection, &settled(), &mut targets);

        assert_eq!(outcome.clamped, 0);
        assert_eq!(targets[neck], -3.0);
    }

    /// Heat softens; it does not stop. And it takes a *cool* reading to come back, not a
    /// missing one — a bus that has stopped answering must not talk a hot joint down.
    #[test]
    fn a_hot_joint_is_flagged_and_only_a_cool_reading_clears_it() {
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());
        let mut temps = [32.0; NUM_JOINTS];
        // The guard edits targets in place, so it needs somewhere to edit: a mutable
        // reference to the const would be a copy nobody reads back.
        let mut targets = DEFAULT_POSITION;

        let outcome = tick(&mut protection, &settled(), &mut targets);
        assert_eq!(outcome.hot, 0, "nothing is hot at 32 °C");

        temps[knee] = 61.0;
        protection.observe_thermals(&temps);
        let outcome = tick(&mut protection, &settled(), &mut targets);
        assert!(outcome.is_hot(knee));
        assert_eq!(protection.overheats[knee], 1);

        // Hysteresis: 55 °C is below the threshold but not below the clear margin.
        temps[knee] = 55.0;
        protection.observe_thermals(&temps);
        let outcome = tick(&mut protection, &settled(), &mut targets);
        assert!(outcome.is_hot(knee), "55 °C must not clear a 60 °C latch");

        // A missing reading is not a cool one.
        temps[knee] = 0.0;
        protection.observe_thermals(&temps);
        let outcome = tick(&mut protection, &settled(), &mut targets);
        assert!(outcome.is_hot(knee), "0 °C is 'no reading', not 'cold'");

        temps[knee] = 49.0;
        protection.observe_thermals(&temps);
        let outcome = tick(&mut protection, &settled(), &mut targets);
        assert!(
            !outcome.is_hot(knee),
            "below the clear margin it must clear"
        );
        assert!(!protection.is_hot(knee));
    }

    /// A hot joint does not stop the robot: the guard reports the heat, and the target still
    /// goes through unchanged. Softening is the *gain*'s job, and `Safety` writes it.
    #[test]
    fn heat_does_not_move_the_target() {
        let knee = joint_index("left_knee").unwrap();
        let mut protection = Protection::new(ProtectionConfig::default());
        let mut temps = [32.0; NUM_JOINTS];
        temps[knee] = 75.0;
        protection.observe_thermals(&temps);

        // A reachable target: the travel clamp is not what this test is about.
        let mut wanted = DEFAULT_POSITION;
        wanted[knee] = 0.5;
        crate::model::clamp_joint_targets(&mut wanted);
        let mut targets = wanted;
        let outcome = tick(&mut protection, &settled(), &mut targets);

        assert!(outcome.is_hot(knee));
        assert_eq!(targets, wanted);
    }

    /// The journal cannot be flooded by a joint that is stuck forever: one line per joint per
    /// cooldown, while the counter keeps counting what the log left out.
    #[test]
    fn a_permanently_stuck_joint_warns_at_most_once_per_cooldown() {
        let neck = joint_index("neck_pitch").unwrap();
        let mut protection = protection_without_the_travel_clamp();

        let mut wanted = DEFAULT_POSITION;
        wanted[neck] = -2.05;
        let sensors = blocked_at(neck, -0.8378, 1400.0);

        // 75 s of a joint pinned solid: ~50 stall entries, and the rate limiter is the only
        // thing standing between them and the journal.
        const TICKS: u32 = 50 * 75;
        for _ in 0..TICKS {
            let mut targets = wanted;
            tick(&mut protection, &sensors, &mut targets);
        }

        assert!(protection.stalls[neck] >= 50, "{}", protection.stalls[neck]);
        // One stall line per joint per cooldown, and at most one release line per *reported*
        // stall — so 75 s of a joint pinned solid is at most 16 lines, and at least a
        // handful. Rate limiting, not silence.
        assert!(
            protection.stall_warnings[neck] <= 16,
            "{} lines for {} stalls",
            protection.stall_warnings[neck],
            protection.stalls[neck]
        );
        assert!(
            protection.stall_warnings[neck] >= 6,
            "rate limiting must not silence the fault: {}",
            protection.stall_warnings[neck]
        );
    }

    /// A robot that is driving normally never touches any of it — the property the rest of
    /// the invariant rests on. Every joint loaded, moving, and following its target.
    #[test]
    fn an_ordinary_robot_is_never_touched() {
        let mut protection = Protection::new(ProtectionConfig::default());
        let temps = [38.0; NUM_JOINTS];
        protection.observe_thermals(&temps);

        // Home as it reaches the servos: the neck's own window is narrower than the table
        // entry, and that one joint is the whole reason the clamp exists.
        let mut home = DEFAULT_POSITION;
        crate::model::clamp_joint_targets(&mut home);

        for step in 0..500 {
            let mut sensors = settled();
            for joint in 0..NUM_JOINTS {
                sensors.velocities[joint] = 1.5 * if step % 2 == 0 { 1.0 } else { -1.0 };
                sensors.currents_ma[joint] = 400.0;
                sensors.pwms[joint] = 400.0;
            }
            let mut targets = home;
            let outcome = tick(&mut protection, &sensors, &mut targets);

            assert_eq!(outcome.held, 0, "a walking robot was held at tick {step}");
            assert_eq!(
                outcome.hot, 0,
                "a walking robot was called hot at tick {step}"
            );
            assert_eq!(targets, home);
        }
        assert_eq!(protection.stalls, [0; NUM_JOINTS]);
    }
}
