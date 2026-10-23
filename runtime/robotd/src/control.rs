//! 主控动作调度与目标生成。
//!
//! 根据命令与动作状态选择模型、组织观测、处理动作历史，
//! 将推理结果换算为关节目标，并应用头部和腿部滤波。

use duck_control::model::{DEFAULT_POSITION, JOINT_SIGN, NUM_JOINTS};
use duck_control::obs::{ACTION_LEN, Command, Observation};
use duck_control::policy::{Net, Policy, PolicyError};

/// Joint indices the head low-pass covers: neck_pitch, head_pitch, head_yaw, head_roll.
const HEAD_JOINTS: std::ops::Range<usize> = 5..9;

/// The ground pick hands back at this fraction of its cycle — the prototype's cutoff.
const GROUND_PICK_END_PHASE: f64 = 0.7;

/// How long the sitstand network rises (posture flag 0) before the main policy takes over.
/// 1 s is enough on the robot — velstand owns the tail of the rise fine.
const RISE_SECS: f64 = 1.0;

/// How recently a roulade request must have arrived, at the end of a roll, for another to
/// chain. The prototype chains on "X still held at the window boundary"; here the client
/// holds the button by re-sending the request every tick, so "held" is "a request landed
/// within the last few ticks". 150 ms is seven ticks — generous against a dropped packet,
/// far too short to mistake a fresh press for a hold.
const ROULADE_CHAIN_WINDOW: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// Scales raw policy output before it becomes a joint offset. The prototype's current
    /// alpha default.
    pub action_scale: f64,
    /// The standing policy is trained to be applied whole.
    pub standing_action_scale: f64,
    /// Standing runs softer, at this fraction of the running gain. `--standing-kp-ratio`.
    pub standing_gain_ratio: f64,
    pub gain: u16,
    /// First-order low-pass on the head joints. `None` is no filtering. The alpha policies
    /// are trained with 0.5 — it must match training or transfer degrades.
    pub head_lowpass: Option<f64>,
    /// Same, for the ten leg joints. Trained with 0.7.
    pub legs_lowpass: Option<f64>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            action_scale: 0.9,
            standing_action_scale: 1.0,
            standing_gain_ratio: 0.8,
            gain: 200,
            head_lowpass: Some(0.5),
            legs_lowpass: Some(0.7),
        }
    }
}

/// The scripted-skill numbers, resolved per mode by `params`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillTuning {
    /// One ground-pick cycle, seconds.
    pub ground_pick_period: f64,
    pub ground_pick_action_scale: f64,
    /// Gain multiplier while the pick runs.
    pub ground_pick_gain_ratio: f64,
    /// How long a kick window stays on the kick network, seconds.
    pub kick_duration: f64,
    /// One roulade — one forward roll, seconds. The prototype's measured single-roll time.
    pub roulade_duration: f64,
    /// Action scale while a roulade runs.
    pub roulade_action_scale: f64,
    /// Gain multiplier while a roulade runs.
    pub roulade_gain_ratio: f64,
}

impl Default for SkillTuning {
    fn default() -> Self {
        Self {
            ground_pick_period: 4.0,
            ground_pick_action_scale: 1.0,
            ground_pick_gain_ratio: 1.0,
            kick_duration: 0.5,
            roulade_duration: 1.0,
            roulade_action_scale: 1.0,
            roulade_gain_ratio: 1.0,
        }
    }
}

/// One tick's worth of decisions, for the caller to act on and report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub targets: [f64; NUM_JOINTS],
    /// Which network drove, as the wire label: `walk`, `stand`, `ground_pick`, `kick_left`,
    /// `kick_right`, `sit`, `rise`.
    pub label: &'static str,
    /// What the gain should be for this tick.
    pub gain: u16,
    /// A scripted move is mid-flight — the robot is moving regardless of the twist, so
    /// restarting the daemon now would put it on the floor.
    pub busy: bool,
}

/// Where the robot is in the sit↔stand cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sit {
    Up,
    /// The sitstand network holds the seat (posture flag 1).
    Sitting,
    /// The sitstand network rises (posture flag 0) for the remaining seconds, then the
    /// main policy takes over.
    Rising {
        remaining: f64,
    },
}

pub struct Controller {
    xgo: crate::xgo::Flow,
    head_ground: crate::xgo::HeadGroundDetector,
    xgo_command: (Command, f64),
    xgo_revision: Option<u64>,
    policy: Policy,
    tuning: Tuning,
    skills: SkillTuning,
    /// Raw previous policy output, which the observation feeds back. Raw, not scaled: the
    /// policy was trained observing its own output, not the actuator command derived from
    /// it. Shared across every network, as the prototype shares it.
    last_action: [f32; ACTION_LEN],
    /// Previous filtered targets, kept only for the low-pass. `None` until the first tick,
    /// so the filter starts from reality rather than dragging up from zero.
    previous: Option<[f64; NUM_JOINTS]>,
    /// Ground-pick phase, 0..[`GROUND_PICK_END_PHASE`]. `None` when inactive.
    ground_pick: Option<f64>,
    /// An active kick window: which leg, and seconds remaining.
    kick: Option<(bool, f64)>,
    /// An active roulade: seconds remaining in the current roll.
    roulade: Option<f64>,
    /// Seconds since a roulade request would still count as "the button is held" — counts
    /// down every tick, refreshed by each request that arrives while a roll runs. At the
    /// end of a roll, positive means chain another.
    roulade_chain: f64,
    sit: Sit,
}

impl Controller {
    pub fn new(policy: Policy, tuning: Tuning, skills: SkillTuning) -> Self {
        Self {
            xgo: crate::xgo::Flow::default(),
            head_ground: Default::default(),
            xgo_command: (Command::default(), 0.0),
            xgo_revision: None,
            policy,
            tuning,
            skills,
            last_action: [0.0; ACTION_LEN],
            previous: None,
            ground_pick: None,
            kick: None,
            roulade: None,
            roulade_chain: 0.0,
            sit: Sit::Up,
        }
    }

    /// Reset the feedback state.
    ///
    /// Called when the policy is re-enabled, so a robot that sat disabled for a minute does
    /// not resume with a stale action in its observation and a filter anchored to wherever
    /// it was before.
    pub fn reset(&mut self) {
        self.last_action = [0.0; ACTION_LEN];
        self.previous = None;
    }

    pub fn has_sitstand(&self) -> bool {
        self.policy.has_sitstand()
    }

    pub fn reset_xgo(&mut self) {
        self.xgo.reset(); self.head_ground.reset(); self.reset();
        self.xgo_command = (Command::default(),0.0);
        self.xgo_revision = None;
    }

    pub fn command_xgo(&mut self, command: Command, mouth: f64, revision: u64) {
        if self.xgo_revision != Some(revision) {
            self.xgo_revision=Some(revision);
            if self.xgo.pick_phi.is_none() { self.xgo_command=(command,mouth); }
        }
    }

    pub fn invalidate_xgo(&mut self) { self.xgo.invalidate(); self.head_ground.reset(); }

    pub fn xgo_recovering(&self) -> bool { self.xgo.phase != crate::xgo::Phase::Walk }

    pub fn start_xgo_pick(&mut self) -> Result<(), &'static str> {
        if !self.policy.has_ground_pick() { return Err("no ground-pick policy loaded"); }
        self.xgo.start_pick()
    }

    pub fn start_xgo_roulade(&mut self, now: f64) -> Result<bool, &'static str> {
        if !self.policy.has_roulade() { return Err("no roulade policy loaded"); }
        self.xgo.start_roulade(now)
    }

    /// Reference Controller.step, with the existing calibrated sensor/model adapter.
    pub fn step_xgo(&mut self, sensors: &duck_control::Sensors,
                    now: f64) -> Result<(Step, f64), PolicyError> {
        let (command,mouth)=self.xgo_command;
        let eligible = self.xgo.pick_phi.is_none() && !self.xgo.rolling();
        let contact = self.head_ground.update(sensors.imu.gravity, &sensors.positions,
            &sensors.velocities, sensors.imu.gyro, self.previous.as_ref(), now, eligible);
        let frame = self.xgo.frame_with_head(sensors.imu.gravity, now, true,
                                  command.twist, command.head, mouth, contact)
            .ok_or_else(|| PolicyError::Inference("invalid xgoduck gravity".to_owned()))?;
        if frame.reset || frame.zero_action { self.reset(); }
        let net = match frame.network {
            crate::xgo::Network::Walk => Net::Walk,
            crate::xgo::Network::Getup => Net::SitStand,
            crate::xgo::Network::Pick => Net::GroundPick,
            crate::xgo::Network::Roulade => Net::Roulade,
        };
        let effective = Command { twist: frame.twist, head: frame.head, ..Command::default() };
        let mut trace_observation=None;
        let action = if frame.zero_action { [0.0; ACTION_LEN] } else {
            let obs = Observation::build(&sensors.imu, &sensors.positions, &sensors.velocities,
                &DEFAULT_POSITION, &self.last_action, &effective);
            trace_observation=Some(obs);
            self.policy.infer(&obs, net)?
        };
        self.last_action = action;
        let offsets = Observation::scatter_action(&action);
        let mut targets = std::array::from_fn(|j|
            DEFAULT_POSITION[j] + JOINT_SIGN[j] * offsets[j]);
        // Upstream retains 0.45 of the previous action and 0.55 of this one.
        // The reference uses the same alpha for head and legs by default.
        let tuning = xgo_filter_tuning(self.tuning, frame.rolling);
        smooth_targets(&mut targets, self.previous, &tuning);
        self.previous = Some(targets);
        duck_control::io_trace::inference(trace_observation.as_ref().map(|o|o.as_slice()),
            &action,sensors,&targets,match net {Net::Walk=>"walk",Net::SitStand=>"sitstand",Net::GroundPick=>"ground_pick",Net::Roulade=>"roulade",_=>"other"},frame.reset || frame.zero_action);
        if !frame.zero_action { self.xgo.inferred(&frame); }
        if frame.picking { self.xgo_command.1=frame.mouth; }
        let label = match self.xgo.phase {
            crate::xgo::Phase::DefaultPose => "xgo_default_pose",
            crate::xgo::Phase::Getup => "xgo_getup",
            _ if frame.rolling => "xgo_roulade",
            _ if frame.picking => "xgo_pick",
            _ => "xgo_walk",
        };
        Ok((Step { targets, label, gain: self.tuning.gain,
                   busy: frame.picking || frame.rolling || self.xgo.phase != crate::xgo::Phase::Walk }, frame.mouth))
    }

    pub fn is_sitting(&self) -> bool {
        self.sit == Sit::Sitting
    }

    /// A scripted move is mid-flight. Sitting itself is not busy — a seated robot is
    /// parked, not travelling.
    pub fn busy(&self) -> bool {
        self.xgo.rolling() || self.xgo.pick_phi.is_some()
            || self.ground_pick.is_some()
            || self.kick.is_some()
            || self.roulade.is_some()
            || matches!(self.sit, Sit::Rising { .. })
    }

    /// Start a one-shot ground pick. The prototype gates the trigger on nothing but the
    /// network existing and the move not already running — a pick can even preempt a kick's
    /// tail, and that stays as it was.
    pub fn start_ground_pick(&mut self) -> Result<(), &'static str> {
        if !self.policy.has_ground_pick() {
            return Err("no ground-pick policy loaded");
        }
        if self.ground_pick.is_some() {
            return Err("ground pick already running");
        }
        self.ground_pick = Some(0.0);
        Ok(())
    }

    /// Start a kick window. Blocked while any scripted move runs, as the prototype blocks it.
    pub fn start_kick(&mut self, left: bool) -> Result<(), &'static str> {
        if !self.policy.has_kick(left) {
            return Err("no kick policy loaded for that leg");
        }
        if self.kick.is_some() || self.ground_pick.is_some() || self.roulade.is_some() {
            return Err("a scripted move is already running");
        }
        self.kick = Some((left, self.skills.kick_duration));
        Ok(())
    }

    /// A roulade request: start a roll, or — arriving while one runs — keep the chain alive.
    ///
    /// `Ok(true)` started a roll; `Ok(false)` refreshed a running one (the caller should
    /// stay quiet: a held button lands here fifty times a second). The prototype gates the
    /// X press on nothing but the ground pick — a roulade can preempt a kick's tail or roll
    /// out of the seat, and both stay as they were.
    pub fn request_roulade(&mut self) -> Result<bool, &'static str> {
        if self.roulade.is_some() {
            self.roulade_chain = ROULADE_CHAIN_WINDOW;
            return Ok(false);
        }
        if !self.policy.has_roulade() {
            return Err("no roulade policy loaded");
        }
        if self.ground_pick.is_some() {
            return Err("a ground pick is running");
        }
        self.roulade = Some(self.skills.roulade_duration);
        self.roulade_chain = 0.0;
        Ok(true)
    }

    /// Sit if standing, stand if sitting. Refused mid-rise, as the prototype refuses it
    /// while a stand transition is in flight.
    pub fn sit_toggle(&mut self) -> Result<&'static str, &'static str> {
        match self.sit {
            Sit::Up => {
                if !self.policy.has_sitstand() {
                    return Err("no sitstand policy loaded");
                }
                self.sit = Sit::Sitting;
                Ok("sit")
            }
            Sit::Sitting => {
                self.sit = Sit::Rising {
                    remaining: RISE_SECS,
                };
                Ok("stand")
            }
            Sit::Rising { .. } => Err("already standing up"),
        }
    }

    /// Engage the sit for the shutdown sequence. The caller owns the timing (sit for a few
    /// seconds, then cut torque and power off); this just puts the sitstand network in
    /// charge with the posture flag at 1.
    pub fn begin_shutdown_sit(&mut self) {
        self.sit = Sit::Sitting;
    }

    /// Seated boot: the robot powered on already sitting, so rise via the sitstand network
    /// instead of dragging the legs through a linear ramp to the standing pose.
    pub fn begin_boot_rise(&mut self) {
        self.sit = Sit::Rising {
            remaining: RISE_SECS,
        };
    }

    /// One tick.
    ///
    /// `body_active` says a client is holding the body-pose mode: the twist is zeroed and
    /// the standing network drives (by magnitude where it is selectable, forced where it is
    /// reserved), exactly as the prototype's B-button mode behaves.
    ///
    /// `scale_mult` multiplies the action scale — voltage adaptation, 1.0 when off.
    pub fn step(
        &mut self,
        sensors: &duck_control::Sensors,
        command: &Command,
        body_active: bool,
        dt: f64,
        scale_mult: f64,
    ) -> Result<Step, PolicyError> {
        // Expire windows first, so a tick after the deadline runs the next thing rather
        // than one more frame of a finished move — the prototype checks its timers at the
        // same point relative to inference.
        if let Some((_, remaining)) = self.kick
            && remaining <= 0.0
        {
            self.kick = None;
        }
        // The end of a roll is a fork, as the prototype forks it: the button still held
        // (a request landed within the chain window) restarts the window — the policy
        // re-initiates a roll from wherever it landed — and released hands back.
        if let Some(remaining) = self.roulade
            && remaining <= 0.0
        {
            self.roulade = if self.roulade_chain > 0.0 {
                Some(self.skills.roulade_duration)
            } else {
                None
            };
        }
        if let Sit::Rising { remaining } = self.sit
            && remaining <= 0.0
        {
            self.sit = Sit::Up;
        }

        // Re-encode the command for the active skill and pick the network. The priority
        // chain is the prototype's: roulade > kick > ground pick > sit/rise > stand > walk.
        let (net, effective, label) = if self.roulade.is_some() {
            // Trained with every command slot at zero; it rolls as soon as it is switched
            // in, so being selected IS the trigger.
            (Net::Roulade, Command::default(), "roulade")
        } else if let Some((left, _)) = self.kick {
            // The kick networks are trained with every command slot at zero — head and
            // body included, whatever the client is holding.
            let net = if left { Net::KickLeft } else { Net::KickRight };
            let label = if left { "kick_left" } else { "kick_right" };
            (net, Command::default(), label)
        } else if let Some(phase) = self.ground_pick {
            // The twist slots carry the phase encoding; head and body are zero-padded,
            // mirroring the training env's `zero_command_padding`.
            let angle = std::f64::consts::TAU * phase;
            let c = Command {
                twist: [angle.cos(), angle.sin(), 0.0],
                ..Command::default()
            };
            (Net::GroundPick, c, "ground_pick")
        } else {
            let mut c = *command;
            match self.sit {
                // The posture flag rides the twist vx slot: 1 = sit, 0 = stand. Head and
                // body slots stay live — the prototype keeps them in the buffer too.
                Sit::Sitting => {
                    c.twist = [1.0, 0.0, 0.0];
                    (Net::SitStand, c, "sit")
                }
                Sit::Rising { .. } => {
                    c.twist = [0.0; 3];
                    (Net::SitStand, c, "rise")
                }
                Sit::Up => {
                    if body_active {
                        c.twist = [0.0; 3];
                    }
                    let standing = self.policy.will_stand(c.twist_magnitude())
                        || (body_active && self.policy.has_standing());
                    if standing {
                        (Net::Stand, c, "stand")
                    } else {
                        (Net::Walk, c, "walk")
                    }
                }
            }
        };

        let observation = Observation::build(
            &sensors.imu,
            &sensors.positions,
            &sensors.velocities,
            &DEFAULT_POSITION,
            &self.last_action,
            &effective,
        );

        let action = self.policy.infer(&observation, net)?;
        self.last_action = action;

        // Scale and gain follow the active state, recomputed every tick. "Standing tuning"
        // applies whenever the *effective* command is inside the standing threshold and the
        // standing network exists — which is how a kick window and the sitstand rise end up
        // at standing gain in the prototype, so they do here too.
        let standing_tuned = matches!(net, Net::Stand)
            || (matches!(net, Net::KickLeft | Net::KickRight | Net::SitStand)
                && self.policy.will_stand(effective.twist_magnitude()));
        let (scale, gain) = match net {
            Net::Roulade => (
                self.skills.roulade_action_scale,
                (self.tuning.gain as f64 * self.skills.roulade_gain_ratio).round() as u16,
            ),
            Net::GroundPick => (
                self.skills.ground_pick_action_scale,
                (self.tuning.gain as f64 * self.skills.ground_pick_gain_ratio).round() as u16,
            ),
            Net::SitStand => (
                // The prototype's `start_sit_toggle` pins the scale at 1.0 for the whole
                // sit/rise cycle.
                1.0,
                if standing_tuned {
                    (self.tuning.gain as f64 * self.tuning.standing_gain_ratio).round() as u16
                } else {
                    self.tuning.gain
                },
            ),
            _ if standing_tuned => (
                self.tuning.standing_action_scale,
                (self.tuning.gain as f64 * self.tuning.standing_gain_ratio).round() as u16,
            ),
            _ => (self.tuning.action_scale, self.tuning.gain),
        };
        let scale = scale * scale_mult;

        let offsets = Observation::scatter_action(&action);
        let mut targets = [0.0; NUM_JOINTS];
        for joint in 0..NUM_JOINTS {
            // The sign layer, target half. `DEFAULT_POSITION` is a hardware reading and
            // stays as it is; only the policy's contribution is flipped, so the action —
            // which is in the model's joint sense — lands on the servo in the servo's.
            targets[joint] =
                DEFAULT_POSITION[joint] + scale * JOINT_SIGN[joint] * offsets[joint];
        }

        smooth_targets(&mut targets, self.previous, &self.tuning);
        self.previous = Some(targets);

        // Advance the windows, after the tick that used them — the prototype advances its
        // phase after the motor write.
        if let Some(phase) = self.ground_pick.as_mut() {
            *phase += dt / self.skills.ground_pick_period;
            if *phase >= GROUND_PICK_END_PHASE {
                self.ground_pick = None;
            }
        }
        if let Some((_, remaining)) = self.kick.as_mut() {
            *remaining -= dt;
        }
        if let Some(remaining) = self.roulade.as_mut() {
            *remaining -= dt;
            self.roulade_chain = (self.roulade_chain - dt).max(0.0);
        }
        if let Sit::Rising { remaining } = &mut self.sit {
            *remaining -= dt;
        }

        Ok(Step {
            targets,
            label,
            gain,
            busy: self.busy(),
        })
    }
}


// The reference resets its filtered action to zero, equivalent to HOME targets.
fn xgo_filter_tuning(tuning: Tuning, rolling: bool) -> Tuning {
    // Upstream alpha weights the OLD action; smooth_targets weights the NEW target.
    let new_weight = if rolling { 0.85 } else { 0.55 };
    Tuning { head_lowpass: Some(new_weight), legs_lowpass: Some(new_weight), ..tuning }
}

fn smooth_targets(
    targets: &mut [f64; NUM_JOINTS],
    previous: Option<[f64; NUM_JOINTS]>,
    tuning: &Tuning,
) {
        {
            let previous = previous.unwrap_or(DEFAULT_POSITION);
            if let Some(alpha) = tuning.head_lowpass {
                for joint in HEAD_JOINTS {
                    targets[joint] = alpha * targets[joint] + (1.0 - alpha) * previous[joint];
                }
            }
            if let Some(alpha) = tuning.legs_lowpass {
                for (joint, target) in targets.iter_mut().enumerate() {
                    if HEAD_JOINTS.contains(&joint) || joint == duck_control::model::MOUTH_INDEX {
                        continue;
                    }
                    *target = alpha * *target + (1.0 - alpha) * previous[joint];
                }
            }
        }
}

#[cfg(test)]
mod tests {
    #[test]
    fn roulade_and_getup_match_reference_filter_across_switches_and_resets() {
        let mut previous = None;
        let mut reference = [0.0; NUM_JOINTS];
        for frame in 0..100 {
            let rolling = (10..60).contains(&frame);
            if frame == 30 || frame == 60 { previous = None; reference = [0.0; NUM_JOINTS]; }
            let action: [f64; NUM_JOINTS] = std::array::from_fn(|i|
                if frame == 0 { 1.0 } else { ((frame+i) as f64 * 0.13).sin() });
            let mut targets = std::array::from_fn(|i| DEFAULT_POSITION[i] + action[i]);
            smooth_targets(&mut targets, previous, &xgo_filter_tuning(Tuning::default(), rolling));
            let old_weight = if rolling { 0.15 } else { 0.45 };
            for i in 0..NUM_JOINTS {
                if i == duck_control::model::MOUTH_INDEX { continue; }
                reference[i] = old_weight * reference[i] + (1.0-old_weight) * action[i];
                assert!((targets[i]-DEFAULT_POSITION[i]-reference[i]).abs() < 1e-12,
                    "frame {frame}, joint {i}, rolling {rolling}");
            }
            previous = Some(targets);
        }
    }

    #[test]
    fn reference_filter_matches_from_first_frame_and_after_reset() {
        let tuning = Tuning { head_lowpass: Some(0.55), legs_lowpass: Some(0.55), ..Tuning::default() };
        let mut previous = None;
        let mut reference = [0.0; NUM_JOINTS];
        for frame in 0..40 {
            if frame == 20 { previous = None; reference = [0.0; NUM_JOINTS]; }
            let action: [f64; NUM_JOINTS] = std::array::from_fn(|i| ((frame + i) as f64 * 0.17).sin() * 0.1);
            let mut targets = std::array::from_fn(|i| DEFAULT_POSITION[i] + action[i]);
            smooth_targets(&mut targets, previous, &tuning);
            for i in 0..NUM_JOINTS {
                if i == duck_control::model::MOUTH_INDEX { continue; }
                reference[i] = 0.45 * reference[i] + 0.55 * action[i];
                assert!((targets[i] - DEFAULT_POSITION[i] - reference[i]).abs() < 1e-12);
            }
            previous = Some(targets);
        }
    }
    #[test]
    fn disabled_filter_keeps_target_and_mouth_is_unfiltered() {
        let tuning = Tuning { head_lowpass: None, legs_lowpass: None, ..Tuning::default() };
        let expected = std::array::from_fn(|i| DEFAULT_POSITION[i] + 0.1);
        let mut targets = expected;
        smooth_targets(&mut targets, None, &tuning);
        assert_eq!(targets, expected);
        let tuning = Tuning { head_lowpass: Some(0.55), legs_lowpass: Some(0.55), ..Tuning::default() };
        smooth_targets(&mut targets, None, &tuning);
        assert_eq!(targets[duck_control::model::MOUTH_INDEX], expected[duck_control::model::MOUTH_INDEX]);
    }

    use super::*;

    /// The prototype's **current alpha configuration** — its built-in defaults, which the
    /// installer deliberately passes no flags to override. The filters are ON at the values
    /// the policies are trained with; changing any of these silently changes how the robot
    /// moves relative to the thing it replaces.
    #[test]
    fn the_defaults_match_the_prototype() {
        let t = Tuning::default();
        assert_eq!(t.action_scale, 0.9);
        assert_eq!(t.standing_action_scale, 1.0);
        assert_eq!(t.standing_gain_ratio, 0.8);
        assert_eq!(
            t.head_lowpass,
            Some(0.5),
            "trained with ACTION_LOW_PASS_HEAD_ALPHA"
        );
        assert_eq!(
            t.legs_lowpass,
            Some(0.7),
            "trained with ACTION_LOW_PASS_LEG_ALPHA"
        );

        let s = SkillTuning::default();
        assert_eq!(s.ground_pick_period, 4.0);
        assert_eq!(s.ground_pick_action_scale, 1.0);
        assert_eq!(s.ground_pick_gain_ratio, 1.0);
        assert_eq!(s.kick_duration, 0.5);
        assert_eq!(s.roulade_duration, 1.0, "one roll, the measured time");
        assert_eq!(s.roulade_action_scale, 1.0);
        assert_eq!(
            s.roulade_gain_ratio, 1.0,
            "a roll runs at full walking gain"
        );
    }

    /// Standing must drop the gain. Running the standing policy at walking stiffness is a
    /// visibly different robot, and the ratio is the prototype's.
    #[test]
    fn standing_softens_the_gain() {
        let t = Tuning::default();
        let standing_gain = (t.gain as f64 * t.standing_gain_ratio).round() as u16;
        assert_eq!(standing_gain, 160);
        assert!(standing_gain < t.gain);
    }

    /// The ground pick ends at 70% of its cycle — ending at 100% replays the reach on the
    /// way out, which is the prototype bug the 0.7 cutoff fixed there.
    #[test]
    fn the_ground_pick_cutoff_is_the_prototypes() {
        assert_eq!(GROUND_PICK_END_PHASE, 0.7);
        assert_eq!(RISE_SECS, 1.0);
    }
}
