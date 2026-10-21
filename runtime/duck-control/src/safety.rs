//! 主控目标检查与硬件写入封装。
//!
//! 检查非有限目标、应用目标范围、跟踪命令时效和倒地状态，
//! 将关节保护结果交给 RobotIo。是否根据倒地状态切换动作由上层控制。

use std::time::Duration;

use crate::io::{IoError, JointTargets, RobotIo, Sensors};
use crate::model::NUM_JOINTS;
use crate::obs::Command;
use crate::protection::{Guard, Protection, ProtectionConfig};

/// The XL330's position range: one turn, centred, from the count↔radian conversion.
///
/// This is the *actuator's* travel, not a joint limit. The per-joint travel —
/// [`crate::model::JOINT_SAFE_RANGE`], built from the mechanism's own joint ranges — is
/// applied alongside it, and is the tighter of the two for every joint. This one stays as
/// the outer bound because it is what the encoder can represent: a target past ±π is a
/// position the servo has no counts for.
pub const ACTUATOR_MIN: f64 = -std::f64::consts::PI;
pub const ACTUATOR_MAX: f64 = std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafetyConfig {
    /// Projected-gravity z above which the robot counts as falling. Upright reads about
    /// -1.0 and a robot on its side reads near 0.
    pub fall_gravity_z: f64,
    /// How long that must hold before it counts. Debounced so a hard footfall is not a
    /// fall.
    pub fall_debounce: Duration,
    /// Intent age past which the velocity command is zeroed.
    pub deadman: Duration,
    /// Gain while running.
    pub gain_running: u16,
    /// Gain to yield at rather than fight the floor. Nothing here applies it — `robotd`
    /// commands it during limp-fall — but it lives with the other safety numbers because
    /// it is one: the gain at which the robot stops pushing back.
    pub gain_limp: u16,
    /// The per-joint guards: travel, stall, heat. See [`crate::protection`].
    pub protection: ProtectionConfig,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            // The prototype's numbers.
            fall_gravity_z: -0.5,
            fall_debounce: Duration::from_millis(200),
            deadman: Duration::from_millis(500),
            gain_running: 200,
            gain_limp: 50,
            protection: ProtectionConfig::default(),
        }
    }
}

/// Why a commanded value was not applied as asked. Surfaced so a client can be told,
/// rather than watching the robot ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Intents went stale; the velocity was zeroed.
    Deadman,
    /// A target was outside the actuator's or the joint's travel.
    Range,
    /// A target was `NaN` or infinite.
    NotFinite,
    /// A joint was pushing without moving, and is being held where it is.
    Stall,
    /// A joint is over its thermal threshold and is running at a reduced gain.
    OverTemp,
}

/// What safety did with a tick's worth of targets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Applied {
    pub limits: Vec<Limit>,
}

impl Applied {
    pub fn limited_by(&self, limit: Limit) -> bool {
        self.limits.contains(&limit)
    }
}

pub struct Safety<T: RobotIo> {
    io: T,
    config: SafetyConfig,
    /// How long gravity has been past the threshold. Reset by any upright sample.
    falling_for: Duration,
    fallen: bool,
    /// Tracks the last gain written so an unchanged one is not rewritten every tick — that
    /// would be fifteen bus writes per tick for no reason.
    gain: Option<u16>,
    /// The per-joint guards. Here rather than in `robotd` because this is the last place a
    /// target exists before it is a bus transaction, and a guard that a caller can forget to
    /// run is not a guard. See [`crate::protection`].
    protect: Protection,
    /// Per-joint gains written *against* the running gain — the thermal guard's exceptions.
    /// `None` is a joint running at [`Self::gain`], which is every joint until one is hot.
    /// Kept so that a softening is written once per transition instead of once per tick.
    soft: [Option<u16>; NUM_JOINTS],
}

impl<T: RobotIo> Safety<T> {
    pub fn new(io: T, config: SafetyConfig) -> Self {
        let mut protect = Protection::new(config.protection);
        protect.set_target_ranges(io.target_ranges());
        Self {
            io,
            config,
            falling_for: Duration::ZERO,
            fallen: false,
            gain: None,
            protect,
            soft: [None; NUM_JOINTS],
        }
    }

    pub fn read(&mut self) -> Result<Sensors, IoError> {
        self.io.read()
    }

    /// Bus reads that are not part of the tick, passed through rather than exposing the IO.
    ///
    /// Safety owns the only `RobotIo` handle — that is what makes "nothing commands a motor
    /// except through here" a fact the borrow checker enforces rather than a convention. Slow
    /// sensors and IMU diagnostics read rather than write, so they are no threat to that, but
    /// handing out the handle to reach them would be.
    ///
    /// The temperatures are also the thermal guard's only input, so this is where it learns
    /// them: every path that reads a servo's case goes through here, and a guard fed by hand
    /// at each call site is a guard that is eventually fed nothing. A failed read leaves the
    /// last sample in place, which keeps a hot joint hot — the verdict is never cleared by a
    /// bus that stopped answering.
    pub fn slow_sensors(&mut self) -> Result<crate::io::SlowSensors, IoError> {
        let slow = self.io.slow_sensors()?;
        self.protect.observe_thermals(&slow.temps_c);
        Ok(slow)
    }

    pub fn imu_stale(&self) -> crate::io::ImuStale {
        self.io.imu_stale()
    }

    pub fn imu_ready(&self) -> bool {
        self.io.imu_ready()
    }

    pub fn fallen(&self) -> bool {
        self.fallen
    }

    /// Power the joints, so the positions this writes can actually be held.
    ///
    /// Through here because [`Safety`] owns the only [`RobotIo`] handle — the same reason
    /// [`Self::slow_sensors`] is a passthrough. Unlike that one this *does* affect the robot, so it
    /// is worth being explicit about what it is and is not:
    ///
    ///  - It is called when a human enables the policy on a limp robot, once, not per tick.
    ///  - It is **not** called at startup. A `robotd` restarted by an update must leave a standing
    ///    robot standing, and nothing here changes that.
    ///  - It does not bypass anything. Torque decides whether the motors hold what
    ///    [`Self::apply`] writes; every clamp, the fall gate and the limp gain still apply to *what*
    ///    gets written. A fallen robot with torque on is still commanded `hold` at `gain_limp`.
    pub fn set_torque(&mut self, on: bool) -> Result<(), IoError> {
        tracing::warn!(on, "torque");
        self.io.set_torque(on)
    }

    /// The gain last written to the servos, or `None` before the first write. This is what
    /// the robot is running at, which is not always what the caller asked for.
    pub fn gain(&self) -> Option<u16> {
        self.gain
    }

    /// Update fall state from a fresh sample. Call every tick, before [`Self::apply`].
    ///
    /// Debounced in both directions: a robot has to be down for `fall_debounce` to count as
    /// fallen, and one upright sample clears the accumulator. Without the debounce, the
    /// impulse from a firm footfall reads as a fall.
    pub fn observe(&mut self, sensors: &Sensors, dt: Duration) {
        // Before the fall gate below, and unconditionally: the stall guard needs this sample
        // whether or not the orientation filter has converged, and it is the *positions* it
        // reads, not the attitude.
        self.protect.observe(sensors, dt);

        // An orientation filter that has not converged does not get a vote.
        //
        // The SFLP filter needs a few seconds of samples before its quaternion means
        // anything, and until then projected gravity is not `[0, 0, -1]` — it is whatever the
        // filter is mid-way through deciding, which reads as "above the fall threshold", which
        // reads as "on its side". Two hundred milliseconds of that and an upright robot on a
        // bench latches `fallen` at startup: `apply` writes `gain_limp`, the policy is refused
        // with "the robot is down; stand it up first", and it clears itself a few seconds
        // later leaving the gain behind at 50 with nothing to explain it.
        //
        // Observed on a board: a joint set to 137 by hand read back 50 after five seconds of
        // robotd, while `robotctl monitor` — sampled later, after convergence — reported `ok`.
        //
        // Holding the previous verdict is the safe default in both directions. At startup that
        // is "not fallen", which is what the robot standing on the bench actually is; and a
        // filter that stops being ready mid-run leaves a fallen robot fallen.
        if !self.io.imu_ready() {
            return;
        }

        let down = sensors.imu.gravity[2] > self.config.fall_gravity_z;
        if down {
            self.falling_for = self.falling_for.saturating_add(dt);
            if self.falling_for >= self.config.fall_debounce {
                self.fallen = true;
            }
        } else {
            self.falling_for = Duration::ZERO;
            self.fallen = false;
        }
    }

    /// Apply the deadman to a command.
    ///
    /// Zeroes the *twist* only. Head targets are left alone deliberately: a stale head pose
    /// is harmless, while a stale velocity walks the robot into a wall.
    pub fn gate(&self, command: Command, intent_age: Duration) -> (Command, Option<Limit>) {
        if intent_age <= self.config.deadman {
            return (command, None);
        }
        let mut stopped = command;
        stopped.twist = [0.0; 3];
        (stopped, Some(Limit::Deadman))
    }

    /// The only path to the motors.
    ///
    /// `hold` is what to command when the policy must not drive — normally the pose the
    /// robot is already in.
    /// `running_gain` is what the caller wants — the standing policy runs softer than the
    /// walking one, and limp-fall softer still. All of those are control decisions, not
    /// safety ones, so they are passed in rather than second-guessed here.
    ///
    /// What happens between the caller's numbers and the servos, in order:
    ///
    ///  1. the gain, if it changed;
    ///  2. `NaN` refused outright;
    ///  3. every joint clamped into [`crate::model::JOINT_SAFE_RANGE`];
    ///  4. a joint that is pushing without moving held where it is
    ///     ([`crate::protection`]);
    ///  5. a joint that is hot run at `overtemp_gain`;
    ///  6. written.
    pub fn apply(
        &mut self,
        targets: [f64; NUM_JOINTS],
        hold: [f64; NUM_JOINTS],
        running_gain: u16,
    ) -> Result<Applied, IoError> {
        let mut applied = Applied::default();

        // Note what is *not* here: a fall gate. Being down does not stop the caller
        // driving, because the verdict is a report (see the module docs). What a fall is
        // worth doing about is decided above, and arrives as ordinary targets and an
        // ordinary gain.
        self.set_gain(running_gain)?;

        // Non-finite is refused, not clamped. Clamping `NaN` silently produces a boundary
        // value, which is a plausible-looking joint angle — far worse than declining to
        // move, because the robot would lurch to a limit rather than hold still.
        //
        // The fallback pose is not exempt from the guards. `hold` is `DEFAULT_POSITION` on
        // several paths through `robotd`, and a pose is only reachable because someone
        // measured it — the neck entry was 69° past its stop until A-6 re-measured the head on
        // 2026-09-12. A refusal that writes a stale pose at full gain is the accident with an
        // extra step, so the clamp and the guards run on this path too.
        if targets.iter().any(|v| !v.is_finite()) {
            applied.limits.push(Limit::NotFinite);
            let mut safe = hold;
            let guard = self.protect.guard(&mut safe);
            self.apply_guard(&guard)?;
            self.io.write(&JointTargets::new(safe))?;
            return Ok(applied);
        }

        let mut safe = targets;
        let guard = self.protect.guard(&mut safe);
        self.apply_guard(&guard)?;

        if guard.clamped != 0 {
            applied.limits.push(Limit::Range);
        }
        if guard.held != 0 {
            applied.limits.push(Limit::Stall);
        }
        if guard.hot != 0 {
            applied.limits.push(Limit::OverTemp);
        }

        self.io.write(&JointTargets::new(safe))?;
        Ok(applied)
    }

    /// Write whatever the per-joint guard implies that is not a target: today, the softer
    /// gain a hot joint runs at.
    ///
    /// Only on a change, like the running gain — a hot joint that stays hot must not cost a
    /// bus transaction per tick, and there is exactly one write when it heats and one when it
    /// cools.
    fn apply_guard(&mut self, guard: &Guard) -> Result<(), IoError> {
        // `set_gain` ran at the top of `apply`, so this is `Some` — but a joint's exception
        // is only meaningful against a known running gain, so refuse to guess.
        let Some(running) = self.gain else {
            return Ok(());
        };
        let soft_gain = self.config.protection.overtemp_gain;
        for joint in 0..NUM_JOINTS {
            let want = if guard.is_hot(joint) {
                Some(soft_gain)
            } else {
                None
            };
            if want == self.soft[joint] {
                continue;
            }
            self.io.set_joint_gain(joint, want.unwrap_or(running))?;
            self.soft[joint] = want;
        }
        Ok(())
    }

    fn set_gain(&mut self, kp: u16) -> Result<(), IoError> {
        if self.gain == Some(kp) {
            return Ok(());
        }
        self.io.set_gain(kp)?;
        self.gain = Some(kp);
        // The global write covers every joint, so nothing is an exception to it any more.
        // A joint that is still hot is re-softened on this tick's `apply_guard`, which is
        // the only place that knows the guard's verdict.
        self.soft = [None; NUM_JOINTS];
        Ok(())
    }

    /// Borrow the wrapped IO. Test-only, and deliberately not public: handing this out in
    /// production would defeat the point of safety owning the writer.
    #[cfg(test)]
    fn io(&self) -> &T {
        &self.io
    }

    /// Mutable access, for a test that has to change what the bus will say next — a servo
    /// that has cooled, say. Test-only for the same reason as [`Self::io`].
    #[cfg(test)]
    fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imu::ImuData;
    use crate::io::FakeIo;
    use crate::model::DEFAULT_POSITION;

    fn upright() -> Sensors {
        Sensors::default() // gravity defaults to [0, 0, -1]
    }

    fn on_its_side() -> Sensors {
        Sensors {
            imu: ImuData {
                gravity: [-1.0, 0.0, 0.0],
                ..ImuData::default()
            },
            ..Sensors::default()
        }
    }

    fn safety() -> Safety<FakeIo> {
        Safety::new(FakeIo::at(DEFAULT_POSITION), SafetyConfig::default())
    }

    /// The home pose *as it actually reaches the servos*: every joint inside its safe travel.
    ///
    /// Tests that mean "the pose the robot is normally in" should mean this one rather than
    /// the raw table, so that a calibration which drifts out of travel fails in one obvious
    /// place instead of turning every "nothing was limited" assertion into a puzzle.
    ///
    /// **Today this is the identity**, because every entry of `DEFAULT_POSITION` is inside its
    /// window since A-6 re-measured the head on 2026-09-12 — the neck entry used to be 69°
    /// past its stop, and holding it off the stop was this clamp's first real job. It is kept
    /// because the notion is what these tests are about, and
    /// [`the_repaired_home_pose_passes_through_untouched`] pins the fact that it currently
    /// changes nothing.
    fn reachable(pose: [f64; NUM_JOINTS]) -> [f64; NUM_JOINTS] {
        let mut out = pose;
        crate::model::clamp_joint_targets(&mut out);
        out
    }

    /// A hard footfall spikes gravity briefly. Treating that as a fall would drop the robot
    /// mid-stride — which is itself how you cause a fall.
    #[test]
    fn a_brief_tilt_is_not_a_fall() {
        let mut s = safety();
        s.observe(&on_its_side(), Duration::from_millis(100));
        assert!(!s.fallen(), "100ms is under the 200ms debounce");
        s.observe(&upright(), Duration::from_millis(20));
        s.observe(&on_its_side(), Duration::from_millis(100));
        assert!(!s.fallen(), "an upright sample must reset the accumulator");
    }

    /// Sustained means fallen.
    #[test]
    fn a_sustained_tilt_is_a_fall() {
        let mut s = safety();
        for _ in 0..11 {
            s.observe(&on_its_side(), Duration::from_millis(20));
        }
        assert!(s.fallen());
    }

    /// Falling must drop the gain, not merely stop commanding. Refusing to command only
    /// freezes the robot in the pose it fell in; going soft lets it yield.
    /// **The regression.** An orientation filter that has not converged must not be able to
    /// declare a robot fallen.
    ///
    /// On a board this cost an afternoon: an upright robot latched `fallen` within 200 ms of
    /// startup, `apply` wrote `gain_limp`, `padd` was refused with "the robot is down; stand
    /// it up first", and a few seconds later it cleared itself — leaving the servos at kP 50
    /// with `robotctl monitor` reporting `ok`, because the gain is only written when it
    /// changes. Nothing in the observable state explained it.
    #[test]
    fn an_unconverged_imu_cannot_declare_a_fall() {
        let mut io = FakeIo::at(DEFAULT_POSITION);
        io.imu_ready = false;
        let mut safety = Safety::new(io, SafetyConfig::default());

        // Gravity well above the threshold — what an unconverged filter reports, and what
        // a robot on its side reports. The difference is only whether the filter is ready.
        let mut sensors = Sensors::default();
        sensors.imu.gravity = [0.0, 0.0, 0.0];

        for _ in 0..50 {
            safety.observe(&sensors, Duration::from_millis(20));
        }
        assert!(
            !safety.fallen(),
            "a robot was called fallen on an orientation filter that had not converged"
        );
    }

    /// And once it has converged, the same sample must be believed — otherwise the guard
    /// above would simply disable fall detection.
    #[test]
    fn a_converged_imu_still_detects_a_fall() {
        let mut io = FakeIo::at(DEFAULT_POSITION);
        io.imu_ready = true;
        let mut safety = Safety::new(io, SafetyConfig::default());

        let mut sensors = Sensors::default();
        sensors.imu.gravity = [0.0, 0.0, 0.0];

        for _ in 0..50 {
            safety.observe(&sensors, Duration::from_millis(20));
        }
        assert!(
            safety.fallen(),
            "a converged filter must still report a fall"
        );
    }

    /// With the gate OFF — the default, the prototype's behaviour — a fall changes
    /// nothing about what gets written: the verdict is reported, the policy keeps
    /// driving, the gain stays the caller's. Going limp under someone adjusting the
    /// robot's lean is exactly the annoyance this default removes.
    #[test]
    fn by_default_a_fall_reports_but_does_not_preempt() {
        let mut s = safety();
        for _ in 0..11 {
            s.observe(&on_its_side(), Duration::from_millis(20));
        }
        assert!(s.fallen(), "the verdict is still tracked");

        let mut wanted = reachable(DEFAULT_POSITION);
        wanted[0] = 0.5;
        let applied = s
            .apply(
                wanted,
                reachable(DEFAULT_POSITION),
                SafetyConfig::default().gain_running,
            )
            .unwrap();
        assert!(applied.limits.is_empty(), "{:?}", applied.limits);
        assert_eq!(
            s.io().last_written.unwrap().positions,
            wanted,
            "the policy keeps driving"
        );
        assert_eq!(s.io().last_gain, Some(SafetyConfig::default().gain_running));
    }

    /// **The fall verdict preempts nothing.** It is tracked, it is published, and it does
    /// not touch the motors: a fallen robot is driven exactly like an upright one, at the
    /// gain the caller asked for.
    ///
    /// This is the contract that lets limp-fall live entirely above this layer — it drops
    /// the gain by *asking*, through the same `apply` as everything else, with no exemption
    /// to special-case. A gate here would have to be bypassed for the pose ramp to move a
    /// robot lying on the floor, and a safety rule with a bypass is not one.
    #[test]
    fn a_fall_does_not_preempt_the_caller() {
        let mut s = safety();
        for _ in 0..11 {
            s.observe(&on_its_side(), Duration::from_millis(20));
        }
        assert!(s.fallen(), "the verdict is still tracked");

        let mut wanted = reachable(DEFAULT_POSITION);
        wanted[0] = 0.5;
        let applied = s
            .apply(
                wanted,
                reachable(DEFAULT_POSITION),
                SafetyConfig::default().gain_running,
            )
            .unwrap();

        assert!(applied.limits.is_empty(), "{:?}", applied.limits);
        assert_eq!(
            s.io().last_written.unwrap().positions,
            wanted,
            "the caller keeps driving a robot that is down"
        );
        assert_eq!(s.io().last_gain, Some(SafetyConfig::default().gain_running));
    }

    /// The other half: a caller that *wants* to go soft says so, and it goes straight
    /// through. This is exactly what `robotd` does during limp-fall.
    #[test]
    fn a_caller_that_asks_for_the_limp_gain_gets_it() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));
        let limp = SafetyConfig::default().gain_limp;

        s.apply(DEFAULT_POSITION, DEFAULT_POSITION, limp).unwrap();
        assert_eq!(s.io().last_gain, Some(limp));
        assert_eq!(
            s.gain(),
            Some(limp),
            "and it is reported as what is running"
        );
    }

    /// `NaN` is refused outright. Clamping it would yield a boundary value — a plausible
    /// joint angle — and the robot would lurch to a limit instead of holding still.
    ///
    /// The fallback pose goes through the guards like anything else. That matters here more
    /// than it looks: `hold` is `DEFAULT_POSITION` on several paths through `robotd`, and a
    /// pose is only reachable because someone measured it — the neck entry was 69° past its
    /// stop until A-6 re-measured the head. A refusal that writes a stale pose at full gain is
    /// the accident with an extra step.
    #[test]
    fn a_non_finite_target_is_refused_not_clamped() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));

        let mut poisoned = DEFAULT_POSITION;
        poisoned[3] = f64::NAN;
        let applied = s
            .apply(
                poisoned,
                DEFAULT_POSITION,
                SafetyConfig::default().gain_running,
            )
            .unwrap();

        assert!(applied.limited_by(Limit::NotFinite));
        assert!(!applied.limited_by(Limit::Range), "must not be clamped");
        assert_eq!(
            s.io().last_written.unwrap().positions,
            reachable(DEFAULT_POSITION),
            "even the fallback is held inside the mechanism"
        );
    }

    /// Out-of-range targets are clamped and reported. Reported matters: a client whose
    /// command was silently altered has no way to know why the robot is not doing as asked.
    ///
    /// The bound that binds is the *joint's* travel table, not ±π — that is the whole point
    /// of A-7. `head_yaw` is the case where the two differ by a wide margin and would have
    /// been written to the actuator's limit before.
    #[test]
    fn out_of_range_targets_are_clamped_and_reported() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));

        let mut wild = DEFAULT_POSITION;
        wild[2] = 100.0;
        wild[7] = -100.0;
        let applied = s
            .apply(wild, DEFAULT_POSITION, SafetyConfig::default().gain_running)
            .unwrap();

        assert!(applied.limited_by(Limit::Range));
        let written = s.io().last_written.unwrap().positions;
        // left_hip_pitch's window ends at the actuator's own limit.
        assert_eq!(written[2], ACTUATOR_MAX);
        // head_yaw's does not, and never did anything but wrap there before.
        assert_eq!(written[7], crate::model::JOINT_SAFE_RANGE[7].0);
        assert!(
            written[7] > ACTUATOR_MIN,
            "and it is not the actuator limit"
        );
    }

    /// **The travel clamp, end to end, on a target that is deliberately out of travel.**
    ///
    /// This test used to build its violation out of the calibration table: the home pose's
    /// neck entry was 69° past where that joint stops, so commanding home *was* commanding an
    /// unreachable angle and the assertion was that the clamp caught it. A-6 re-measured the
    /// head on 2026-09-12 — `DEFAULT_POSITION[5]` is now −0.34668, inside its window — so that
    /// premise is gone. That is the fix, not a regression, and the repair is pinned by
    /// [`the_repaired_home_pose_passes_through_untouched`] below.
    ///
    /// The violation is constructed here instead, which makes the test about the mechanism
    /// rather than about one number in a table: a target past a joint's window is clamped *to*
    /// the window, the rest of the pose is untouched, and the client is told. Every path that
    /// can still command a joint out of travel — a policy output, a stale client, a future
    /// mis-calibration — arrives here.
    #[test]
    fn an_out_of_travel_target_is_clamped_and_the_client_is_told() {
        let neck = crate::model::joint_index("neck_pitch").unwrap();
        let head_yaw = crate::model::joint_index("head_yaw").unwrap();
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));

        // Two joints, one in each direction, both explicitly outside their windows — and
        // `head_yaw`'s is past the actuator's own ±π as well, so both limits are exercised.
        let mut wanted = DEFAULT_POSITION;
        wanted[neck] = crate::model::JOINT_SAFE_RANGE[neck].0 - 0.3;
        wanted[head_yaw] = crate::model::JOINT_SAFE_RANGE[head_yaw].1 + 0.3;

        let applied = s
            .apply(
                wanted,
                DEFAULT_POSITION,
                SafetyConfig::default().gain_running,
            )
            .unwrap();

        assert!(applied.limited_by(Limit::Range), "the client is told");
        let written = s.io().last_written.unwrap().positions;
        assert_eq!(written[neck], crate::model::JOINT_SAFE_RANGE[neck].0);
        assert_eq!(
            written[head_yaw],
            crate::model::JOINT_SAFE_RANGE[head_yaw].1
        );
        assert!(
            written[head_yaw] < ACTUATOR_MAX,
            "the joint's own travel is the bound that binds, not the servo's one turn"
        );
        // Everything else is what it was: this is two joints' problem, not the pose's.
        for (joint, value) in written.iter().enumerate() {
            if joint != neck && joint != head_yaw {
                assert_eq!(*value, DEFAULT_POSITION[joint]);
            }
        }
    }

    /// **The other half, and the half A-6's calibration fix is what made assertable**: an
    /// ordinary tick at home is limited by nothing at all.
    ///
    /// Before the head was re-measured this could not be written down — every tick at home was
    /// clamped at `neck_pitch`, which is exactly what the test above used to be built around.
    /// It is worth its own test rather than a line inside another one, because a limit that
    /// quietly mangles normal operation is the failure mode a per-joint clamp is most likely
    /// to have, and this is the assertion that would catch it.
    #[test]
    fn the_repaired_home_pose_passes_through_untouched() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));

        let applied = s
            .apply(
                DEFAULT_POSITION,
                DEFAULT_POSITION,
                SafetyConfig::default().gain_running,
            )
            .unwrap();

        assert!(
            applied.limits.is_empty(),
            "home is inside every window and must not be limited at all: {:?}",
            applied.limits
        );
        assert_eq!(
            s.io().last_written.unwrap().positions,
            DEFAULT_POSITION,
            "and it is written verbatim"
        );
    }

    /// An ordinary tick must pass through untouched, or the clamp is silently mangling
    /// normal operation and every other test here proves nothing.
    #[test]
    fn an_ordinary_target_passes_through_unchanged() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));

        let pose = reachable(DEFAULT_POSITION);
        let applied = s
            .apply(pose, pose, SafetyConfig::default().gain_running)
            .unwrap();
        assert!(applied.limits.is_empty(), "{:?}", applied.limits);
        assert_eq!(s.io().last_written.unwrap().positions, pose);
        assert_eq!(s.io().last_gain, Some(SafetyConfig::default().gain_running));
    }

    /// A joint that is pushing without moving is held where it is, and the tick says so.
    ///
    /// The end-to-end version of `protection`'s own test: the sample arrives through
    /// `observe`, the thresholds come from `SafetyConfig`, and what the servo gets is checked
    /// at the bus.
    #[test]
    fn a_stalled_joint_is_held_where_it_is_and_reported() {
        let neck = crate::model::joint_index("neck_pitch").unwrap();

        // The joint pinned at its stop: loaded, saturated, not moving.
        let mut sensors = Sensors {
            positions: DEFAULT_POSITION,
            ..Sensors::default()
        };
        sensors.positions[neck] = crate::model::NECK_PITCH_MEASURED_STOP;
        sensors.currents_ma[neck] = 1400.0;
        sensors.pwms[neck] = 880.0;

        // A target well outside the joint's window, with the travel clamp off, so this is the
        // stall guard firing rather than the table. It used to be this robot's home pose for
        // the neck; A-6 re-measured that on 2026-09-12 and nothing here depended on it — the
        // guard answers *any* target the joint cannot reach.
        let mut wanted = DEFAULT_POSITION;
        wanted[neck] = -2.05;
        let mut config = SafetyConfig::default();
        config.protection.limits = false;
        let mut s = Safety::new(FakeIo::at(DEFAULT_POSITION), config);

        let mut last = None;
        for _ in 0..25 {
            s.observe(&sensors, Duration::from_millis(20));
            last = Some(
                s.apply(wanted, DEFAULT_POSITION, config.gain_running)
                    .unwrap(),
            );
        }

        let applied = last.expect("25 ticks ran");
        assert!(applied.limited_by(Limit::Stall), "{:?}", applied.limits);
        assert_eq!(
            s.io().last_written.unwrap().positions[neck],
            crate::model::NECK_PITCH_MEASURED_STOP,
            "the joint is commanded where it is"
        );
    }

    /// A hot joint runs softer, and the softening is a *bus write on the transition*, not a
    /// write per tick: one when it heats, one when it cools, and nothing in between.
    #[test]
    fn a_hot_joint_runs_at_the_softened_gain() {
        let knee = crate::model::joint_index("left_knee").unwrap();
        let mut io = FakeIo::at(DEFAULT_POSITION);
        let mut slow = io.slow.unwrap();
        slow.temps_c[knee] = 65.0;
        io.slow = Some(slow);
        let config = SafetyConfig::default();
        let soft = config.protection.overtemp_gain;
        let mut s = Safety::new(io, config);

        // The temperatures arrive through the slow read — nobody has to remember to feed
        // the guard.
        s.slow_sensors().expect("a plausible robot has a bus");

        s.observe(&upright(), Duration::from_millis(20));
        let pose = reachable(DEFAULT_POSITION);
        let applied = s.apply(pose, pose, config.gain_running).unwrap();

        assert!(applied.limited_by(Limit::OverTemp), "{:?}", applied.limits);
        assert_eq!(s.io().joint_gains[knee], Some(soft));
        assert_eq!(
            s.io().last_written.unwrap().positions,
            pose,
            "targets unaffected"
        );
        for joint in 0..NUM_JOINTS {
            if joint != knee {
                assert_eq!(
                    s.io().joint_gains[joint],
                    None,
                    "{} was softened to cool a knee",
                    crate::model::JOINT_NAMES[joint]
                );
            }
        }

        // Three more hot ticks: no further writes.
        for _ in 0..3 {
            s.observe(&upright(), Duration::from_millis(20));
            s.apply(pose, pose, config.gain_running).unwrap();
        }
        assert_eq!(s.io().joint_gain_writes, 1, "one write per transition");

        // Cooled: the joint goes back to the running gain, in one more write.
        let mut slow = s.io_mut().slow.unwrap();
        slow.temps_c[knee] = 40.0;
        s.io_mut().slow = Some(slow);
        s.slow_sensors().unwrap();
        s.observe(&upright(), Duration::from_millis(20));
        let applied = s.apply(pose, pose, config.gain_running).unwrap();

        assert!(!applied.limited_by(Limit::OverTemp));
        assert_eq!(s.io().joint_gains[knee], Some(config.gain_running));
        assert_eq!(s.io().joint_gain_writes, 2, "one write per transition");
    }

    /// The deadman zeroes the twist and nothing else. Losing comms should make the robot
    /// stand still — not collapse, and not forget where its head was pointing.
    #[test]
    fn the_deadman_zeroes_the_twist_only() {
        let s = safety();
        let command = Command {
            twist: [0.5, 0.0, 0.3],
            head: [0.1, 0.2, 0.3, 0.4],
            ..Command::default()
        };

        let (fresh, limit) = s.gate(command, Duration::from_millis(100));
        assert_eq!(fresh, command, "a fresh intent passes through");
        assert!(limit.is_none());

        let (stale, limit) = s.gate(command, Duration::from_secs(5));
        assert_eq!(stale.twist, [0.0; 3], "velocity must stop");
        assert_eq!(stale.head, command.head, "head is harmless when stale");
        assert_eq!(limit, Some(Limit::Deadman));
    }

    /// The gain is written once per transition, not once per tick. At 50 Hz the naive
    /// version would be 750 extra bus writes a second, on the bus the control loop needs.
    #[test]
    fn the_gain_is_only_written_when_it_changes() {
        let mut s = safety();
        s.observe(&upright(), Duration::from_millis(20));
        s.apply(
            DEFAULT_POSITION,
            DEFAULT_POSITION,
            SafetyConfig::default().gain_running,
        )
        .unwrap();
        s.apply(
            DEFAULT_POSITION,
            DEFAULT_POSITION,
            SafetyConfig::default().gain_running,
        )
        .unwrap();
        s.apply(
            DEFAULT_POSITION,
            DEFAULT_POSITION,
            SafetyConfig::default().gain_running,
        )
        .unwrap();

        // Three applies, three position writes, but only the first gain write.
        assert_eq!(s.io().writes, 3);
        assert_eq!(s.io().last_gain, Some(SafetyConfig::default().gain_running));
        assert_eq!(s.gain, Some(SafetyConfig::default().gain_running));
    }
    #[test]
    fn calibrated_backend_ranges_reach_the_final_safety_writer() {
        struct Calibrated(FakeIo);
        impl RobotIo for Calibrated {
            fn target_ranges(&self) -> Option<[(f64,f64); NUM_JOINTS]> {
                Some(std::array::from_fn(|i| (-4.0-i as f64*0.1, 4.0+i as f64*0.1)))
            }
            fn read(&mut self)->crate::io::Result<Sensors>{self.0.read()}
            fn write(&mut self,t:&JointTargets)->crate::io::Result<()>{self.0.write(t)}
            fn set_gain(&mut self,k:u16)->crate::io::Result<()>{self.0.set_gain(k)}
            fn set_joint_gain(&mut self,j:usize,k:u16)->crate::io::Result<()>{self.0.set_joint_gain(j,k)}
            fn set_torque(&mut self,on:bool)->crate::io::Result<()>{self.0.set_torque(on)}
            fn slow_sensors(&mut self)->crate::io::Result<crate::io::SlowSensors>{self.0.slow_sensors()}
        }
        let mut safety=Safety::new(Calibrated(FakeIo::default()), SafetyConfig::default());
        let mut current=DEFAULT_POSITION;
        current[2]=1.2164+(3878.0-2231.0)*2.0*std::f64::consts::PI/4096.0;
        let applied=safety.apply(current,current,6).unwrap();
        assert!(!applied.limited_by(Limit::Range));
        assert_eq!(safety.io().0.last_written.unwrap().positions,current);
        for side in [-1.0,1.0] {
            let applied=safety.apply([100.0*side;NUM_JOINTS],current,6).unwrap();
            assert!(applied.limited_by(Limit::Range));
            let got=safety.io().0.last_written.unwrap().positions;
            for i in 0..NUM_JOINTS {assert_eq!(got[i],side*(4.0+i as f64*0.1));}
        }
    }
}
