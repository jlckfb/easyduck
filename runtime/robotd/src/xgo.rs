//! Control sequencing ported from xgoduck_runtime_arduino at 8cdbbd84710d.
//! Hardware coordinates and servo writes belong to the existing driver.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase { Walk, DefaultPose, Getup }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Network { Walk, Getup, Pick, Roulade }

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub network: Network,
    pub zero_action: bool,
    pub reset: bool,
    pub twist: [f64; 3],
    pub head: [f64; 4],
    pub mouth: f64,
    pub picking: bool,
    pub rolling: bool,
}

#[derive(Debug)]
pub struct Flow {
    pub phase: Phase,
    pub fall_elapsed: f64,
    pub stand_elapsed: f64,
    pub hold_elapsed: f64,
    phase_since: Option<f64>,
    pub pick_phi: Option<f64>,
    roulade_until: Option<f64>,
    reset_policy: bool,
    mouth: Option<f64>,
}

impl Default for Flow {
    fn default() -> Self {
        Self { phase: Phase::Walk, fall_elapsed: 0.0, stand_elapsed: 0.0,
            hold_elapsed: 0.0, phase_since: None, pick_phi: None, roulade_until: None,
            reset_policy: true, mouth: None }
    }
}


// Joint-angle condition used as a head-contact posture proxy.
// This joint signature is a contact proxy, not a contact sensor or ground-height estimate.
pub fn head_ground_posture(q: &[f64;15]) -> bool {
    q[5..9].iter().all(|v| v.is_finite()) && q[5] <= -2.5
}
pub fn head_ground_condition(gravity:[f64;3], q:&[f64;15]) -> bool {
    let norm=gravity.iter().map(|v|v*v).sum::<f64>().sqrt();
    // Nominal 30 degrees, 2 degrees tolerance for the captured 29.2-degree resting pose.
    norm.is_finite() && norm>1e-6 && gravity[0]/norm>0.0
        && -gravity[2]/norm <= 28.0f64.to_radians().cos() && head_ground_posture(q)
}

/// Contact evidence from posture or sustained failure to lift the neck.
/// This cannot distinguish the floor from another physical obstruction.
#[derive(Default, Debug)]
pub struct HeadGroundDetector {
    blocked_since: Option<(f64, f64)>,
    last_sample: Option<f64>,
}
impl HeadGroundDetector {
    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn update(&mut self, gravity: [f64;3], q: &[f64;15], velocity: &[f64;15],
                  gyro: [f64;3], target: Option<&[f64;15]>, now: f64, eligible: bool) -> bool {
        let norm = gravity.iter().map(|v| v*v).sum::<f64>().sqrt();
        let valid = eligible && now.is_finite() && norm.is_finite() && norm > 1e-6
            && q[5..9].iter().chain(velocity[5..7].iter()).chain(gyro.iter()).all(|v|v.is_finite());
        if !valid { self.reset(); return false; }
        // Missing/old samples and a clock discontinuity cannot count toward obstruction.
        if self.last_sample.is_some_and(|t| now < t || now-t > 0.1) {
            self.blocked_since = None;
        }
        self.last_sample = Some(now);
        let angle = (-gravity[2]/norm).clamp(-1.0,1.0).acos().to_degrees();
        let forward = gravity[0] >= 0.0;
        let quiet = velocity[5..7].iter().all(|v|v.abs() < 0.15)
            && gyro.iter().map(|v|v*v).sum::<f64>().sqrt() < 0.25;
        // Positive neck-coordinate error means the controller is trying to unfold it.
        // Below 28 degrees require the deeply folded posture as additional evidence.
        let blocked = forward && quiet && (angle >= 28.0 || head_ground_posture(q))
            && target.is_some_and(|t| t[5].is_finite() && t[5]-q[5] >= 20.0f64.to_radians());
        if !blocked { self.blocked_since = None; }
        let stuck = if blocked {
            // A slowly following neck is not a stalled neck: bound total movement too.
            match self.blocked_since {
                Some((since, neck)) if (q[5]-neck).abs() <= 3.0f64.to_radians() => now >= since+1.0,
                _ => { self.blocked_since = Some((now,q[5])); false }
            }
        } else { false };
        head_ground_condition(gravity,q) || stuck
    }
}

impl Flow {
    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn invalidate(&mut self) {
        self.fall_elapsed = 0.0;
        self.stand_elapsed = 0.0;
        if self.phase == Phase::DefaultPose {
            self.phase_since = None;
            self.hold_elapsed = 0.0;
        }
    }

    pub fn start_pick(&mut self) -> Result<(), &'static str> {
        if self.phase != Phase::Walk { return Err("pick only while walking upright"); }
        if self.roulade_until.is_some() { return Err("pick unavailable during roulade"); }
        self.pick_phi = Some(0.0);
        self.mouth = Some(1.0);
        self.reset_policy = true;
        Ok(())
    }

    pub fn rolling(&self) -> bool { self.roulade_until.is_some() }

    /// The controller sends a held gamepad button repeatedly. Keep the official 1.9 s
    /// window alive while held, but reset policy history only on the first press.
    pub fn start_roulade(&mut self, now: f64) -> Result<bool, &'static str> {
        if self.phase != Phase::Walk || self.pick_phi.is_some() {
            return Err("roulade only while walking");
        }
        let started = self.roulade_until.is_none();
        self.roulade_until = Some(now + 1.9);
        if started { self.reset_policy = true; }
        Ok(started)
    }

    pub fn frame(&mut self, gravity: [f64; 3], now: f64, valid: bool,
                 twist: [f64; 3], head: [f64; 4], mouth: f64) -> Option<Frame> {
        self.frame_with_head(gravity,now,valid,twist,head,mouth,false)
    }

    /// head_ground is already qualified contact evidence, not just a neck angle.
    pub fn frame_with_head(&mut self, gravity:[f64;3], now:f64, valid:bool,
        twist:[f64;3], head:[f64;4], mouth:f64, head_ground:bool) -> Option<Frame> {
        if self.roulade_until.is_some_and(|until| now >= until) {
            self.roulade_until = None;
            self.fall_elapsed = 0.0;
            self.reset_policy = true;
        }
        let norm = gravity.iter().map(|v| v*v).sum::<f64>().sqrt();
        if !valid || !norm.is_finite() || norm < 1e-6 {
            self.invalidate();
            return None;
        }
        let angle = (-gravity[2]/norm).clamp(-1.0, 1.0).acos().to_degrees();
        let before = self.phase;
        if self.roulade_until.is_none() { match self.phase {
            Phase::Walk => {
                if angle > 45.0 || (head_ground && self.pick_phi.is_none()) {
                    self.fall_elapsed += 1.0/50.0;
                    if self.fall_elapsed >= 0.15 {
                        self.phase = Phase::DefaultPose;
                        self.phase_since = Some(now);
                        self.hold_elapsed = 0.0;
                    }
                } else { self.fall_elapsed = 0.0; }
            },
            Phase::DefaultPose => {
                let since = *self.phase_since.get_or_insert(now);
                self.hold_elapsed = now-since;
                if now >= since+1.0 {
                    self.phase = Phase::Getup;
                    self.stand_elapsed = 0.0;
                }
            },
            Phase::Getup => {
                if angle < 15.0 && !head_ground {
                    self.stand_elapsed += 1.0/50.0;
                    if self.stand_elapsed >= 1.0 {
                        self.phase = Phase::Walk;
                        self.fall_elapsed = 0.0;
                    }
                } else { self.stand_elapsed = 0.0; }
            },
        } }
        if self.phase != before {
            self.reset_policy = true;
            if self.phase != Phase::Walk { self.pick_phi = None; self.roulade_until = None; }
        }
        let rolling = self.roulade_until.is_some() && self.phase == Phase::Walk;
        let picking = self.pick_phi.is_some() && self.phase == Phase::Walk;
        let (network, twist, head) = if rolling {
            (Network::Roulade, [0.0;3], [0.0;4])
        } else if let Some(phi) = self.pick_phi.filter(|_| picking) {
            let angle = std::f64::consts::TAU * phi;
            self.mouth = Some(if phi < 0.4 { 1.0 } else { 0.0 });
            (Network::Pick, [angle.cos(), angle.sin(), 0.0], [0.0;4])
        } else if self.phase == Phase::Getup {
            (Network::Getup, [0.0;3], [0.0;4])
        } else if self.phase == Phase::DefaultPose {
            (Network::Getup, twist, head)
        } else { (Network::Walk, twist, head) };
        // Controller.command ignores mouth updates only while a pick is active.
        if !picking { self.mouth = Some(mouth); }
        let frame = Frame { network, twist, head, mouth: self.mouth.unwrap_or(mouth),
            zero_action: self.phase == Phase::DefaultPose,
            reset: self.reset_policy, picking, rolling };
        Some(frame)
    }

    /// Called only after successful inference, as in Controller.step.
    pub fn inferred(&mut self, frame: &Frame) {
        self.reset_policy = false;
        if frame.picking {
            if let Some(phi) = self.pick_phi.as_mut() {
                *phi += 0.02/4.0;
                if *phi >= 1.0 { self.pick_phi = None; self.reset_policy = true; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pick_runs_200_frames_and_closes_at_point_four() {
        let mut f = Flow::default(); f.start_pick().unwrap();
        for i in 0..200 {
            let frame=f.frame([0.0,0.0,-1.0],i as f64*0.02,true,[0.3,0.0,0.0],[0.2;4],0.5).unwrap();
            assert_eq!(frame.network,Network::Pick);
            assert_eq!(frame.mouth,if i<80 {1.0} else {0.0});
            assert_eq!(frame.head,[0.0;4]);
            f.inferred(&frame);
        }
        assert_eq!(f.pick_phi,None);
        assert_eq!(f.frame([0.0,0.0,-1.0],4.0,true,[0.0;3],[0.0;4],0.0).unwrap().network,Network::Walk);
    }
    #[test]
    fn fall_has_eight_ticks_then_one_second_zero_action() {
        let mut f=Flow::default();
        for i in 0..8 { let r=f.frame([1.0,0.0,0.0],i as f64*0.02,true,[0.0;3],[0.0;4],0.0).unwrap();
            assert_eq!(r.zero_action,i==7); }
        assert!(f.frame([1.0,0.0,0.0],1.139,true,[0.0;3],[0.0;4],0.0).unwrap().zero_action);
        assert_eq!(f.frame([1.0,0.0,0.0],1.141,true,[0.4;3],[0.3;4],0.0).unwrap().network,Network::Getup);
        assert_eq!(f.phase,Phase::Getup);
        for i in 0..50 { f.frame([0.0,0.0,-1.0],1.16+i as f64*0.02,true,[0.0;3],[0.0;4],0.0); }
        assert_eq!(f.phase,Phase::Walk);
    }
    #[test]
    fn invalid_sample_resets_condition_counters_and_hold_anchor() {
        let mut f=Flow::default();
        for i in 0..8 { f.frame([1.0,0.0,0.0],i as f64*0.02,true,[0.0;3],[0.0;4],0.0); }
        f.frame([f64::NAN,0.0,0.0],2.0,true,[0.0;3],[0.0;4],0.0);
        assert!(f.frame([1.0,0.0,0.0],3.0,true,[0.0;3],[0.0;4],0.0).unwrap().zero_action);
        assert!(f.frame([1.0,0.0,0.0],3.99,true,[0.0;3],[0.0;4],0.0).unwrap().zero_action);
    }
    #[test]
    fn roulade_uses_its_policy_then_returns_to_walk_without_getup() {
        let mut f=Flow::default();
        assert_eq!(f.start_roulade(0.0),Ok(true));
        assert!(f.start_pick().is_err());
        for i in 0..94 {
            let frame=f.frame([1.0,0.0,0.0],i as f64*0.02,true,[0.3;3],[0.4;4],0.0).unwrap();
            assert_eq!(frame.network,Network::Roulade);
            assert_eq!(frame.twist,[0.0;3]);
            assert_eq!(frame.head,[0.0;4]);
            assert!(frame.rolling);
            f.inferred(&frame);
        }
        let frame=f.frame([0.0,0.0,-1.0],1.91,true,[0.0;3],[0.0;4],0.0).unwrap();
        assert_eq!(frame.network,Network::Walk);
        assert!(frame.reset);
        assert!(!frame.rolling);
        assert_eq!(f.phase,Phase::Walk);
    }
    #[test]
    fn held_roulade_extends_window_and_a_pick_blocks_it() {
        let mut f=Flow::default();
        assert_eq!(f.start_roulade(0.0),Ok(true));
        assert_eq!(f.start_roulade(1.0),Ok(false));
        assert_eq!(f.frame([0.0,0.0,-1.0],2.0,true,[0.0;3],[0.0;4],0.0).unwrap().network,Network::Roulade);
        assert_eq!(f.frame([0.0,0.0,-1.0],2.91,true,[0.0;3],[0.0;4],0.0).unwrap().network,Network::Walk);
        f.start_pick().unwrap();
        assert!(f.start_roulade(3.0).is_err());
    }
}


#[cfg(test)]
mod head_ground_regression {
    use super::*;
    fn folded_contact(deg:f64)->bool {
        let mut q=[0.0;15];q[5]=-3.2;
        head_ground_condition(gravity(deg),&q)
    }
    fn gravity(deg:f64)->[f64;3] { let a=deg.to_radians();[a.sin(),0.0,-a.cos()] }
    #[test]
    fn captured_head_ground_pose_enters_recovery() {
        let mut q=[0.0;15];q[5]=-3.186028513284673;q[6]=-0.4433126147876595;
        assert!(head_ground_posture(&q));
        let mut f=Flow::default();
        for i in 0..8 {f.frame_with_head(gravity(29.2077),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,head_ground_posture(&q));}
        assert_eq!(f.phase,Phase::DefaultPose);
    }
    #[test]
    fn head_pose_alone_and_ordinary_lean_do_not_trigger() {
        for (deg,contact) in [(10.0,true),(29.2,false),(35.0,false)] {
            let mut f=Flow::default();
            for i in 0..20 { f.frame_with_head(gravity(deg),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,contact && folded_contact(deg)); }
            assert_eq!(f.phase,Phase::Walk);
        }
        let mut f=Flow::default();
        for i in 0..20 {f.frame_with_head(gravity(-30.0),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,folded_contact(-30.0));}
        assert_eq!(f.phase,Phase::Walk);
    }
    #[test]
    fn short_dip_and_pick_do_not_become_a_head_ground_fall() {
        let mut f=Flow::default();
        for i in 0..7 {f.frame_with_head(gravity(30.0),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,true);}
        f.frame_with_head(gravity(10.0),0.14,true,[0.0;3],[0.0;4],0.0,folded_contact(10.0));
        assert_eq!(f.phase,Phase::Walk);
        f.start_pick().unwrap();
        for i in 0..20 {f.frame_with_head(gravity(30.0),1.0+i as f64*0.02,true,[0.0;3],[0.0;4],0.0,true);}
        assert_eq!(f.phase,Phase::Walk);
    }
}

#[cfg(test)]
mod head_obstruction_tests {
    use super::*;
    fn gravity(deg:f64)->[f64;3] { let a=deg.to_radians(); [a.sin(),0.0,-a.cos()] }
    fn update(d: &mut HeadGroundDetector, q: &[f64;15], target: Option<&[f64;15]>,
              now:f64, eligible:bool, deg:f64, speed:f64, gyro:f64) -> bool {
        let mut v=[0.0;15];v[5]=speed;
        d.update(gravity(deg),q,&v,[gyro,0.0,0.0],target,now,eligible)
    }
    #[test]
    fn captured_neck_recovers_across_head_pitch_variations() {
        for pitch in [-2.0,-0.443,0.0,1.0] {
            let mut q=[0.0;15]; q[5]=-3.186028513284673;q[6]=pitch;
            let mut detector=HeadGroundDetector::default(); let mut f=Flow::default();
            for i in 0..8 {
                let now=i as f64*0.02;
                let contact=update(&mut detector,&q,None,now,true,29.2077,0.0,0.0);
                f.frame_with_head(gravity(29.2077),now,true,[0.0;3],[0.0;4],0.0,contact);
            }
            assert_eq!(f.phase,Phase::DefaultPose,"head pitch {pitch}");
            f.frame_with_head(gravity(29.2077),1.141,true,[0.0;3],[0.0;4],0.0,false);
            assert_eq!(f.phase,Phase::Getup);
        }
    }
    #[test]
    fn sustained_lift_obstruction_recovers_other_neck_postures() {
        for (deg,neck) in [(30.0,-1.4),(5.0,-3.18),(0.0,-3.18)] {
            let mut q=[0.0;15];q[5]=neck;q[6]=1.0;
            let mut t=q;t[5]+=0.5;
            let mut d=HeadGroundDetector::default();let mut f=Flow::default();
            for i in 0..58 {
                let now=i as f64*0.02;
                let contact=update(&mut d,&q,Some(&t),now,true,deg,0.0,0.0);
                if i<50 {assert!(!contact);}
                f.frame_with_head(gravity(deg),now,true,[0.0;3],[0.0;4],0.0,contact);
            }
            assert_eq!(f.phase,Phase::DefaultPose,"tilt {deg}, neck {neck}");
        }
    }
    #[test]
    fn upright_trunk_cannot_finish_getup_while_the_head_is_still_blocked() {
        let mut q=[0.0;15];q[5]=-3.18;let mut target=q;target[5]+=0.5;
        let mut detector=HeadGroundDetector::default();let mut flow=Flow::default();
        // Reproduce contact while the trunk is already vertical.
        for i in 0..180 {
            let now=i as f64*0.02;
            let contact=update(&mut detector,&q,Some(&target),now,true,0.0,0.0,0.0);
            flow.frame_with_head(gravity(0.0),now,true,[0.0;3],[0.0;4],0.0,contact);
        }
        assert_eq!(flow.phase,Phase::Getup);
        // Once the head follows the lifting command, require one full upright second.
        q[5]=target[5];
        for i in 0..49 {
            let now=3.6+i as f64*0.02;
            let contact=update(&mut detector,&q,Some(&target),now,true,0.0,0.0,0.0);
            assert!(!contact);
            flow.frame_with_head(gravity(0.0),now,true,[0.0;3],[0.0;4],0.0,contact);
            assert_eq!(flow.phase,Phase::Getup);
        }
        flow.frame_with_head(gravity(0.0),4.6,true,[0.0;3],[0.0;4],0.0,false);
        assert_eq!(flow.phase,Phase::Walk);
    }

    #[test]
    fn following_moving_backwards_and_scripted_actions_do_not_count_as_contact() {
        let mut q=[0.0;15];q[5]=-1.4;let mut t=q;t[5]+=0.5;
        for (target,deg,speed,gyro,eligible) in [
            (Some(&q),30.0,0.0,0.0,true), // normal low head, following its command
            (None,30.0,0.0,0.0,true),
            (Some(&t),30.0,0.3,0.0,true),
            (Some(&t),30.0,0.0,0.4,true),
            (Some(&t),-30.0,0.0,0.0,true),
            (Some(&t),30.0,0.0,0.0,false), // pick/roulade/recovery/invalid IMU
            (Some(&t),5.0,0.0,0.0,true), // ordinary near-upright head posture
        ] {
            let mut d=HeadGroundDetector::default();
            for i in 0..100 {assert!(!update(&mut d,&q,target,i as f64*0.02,eligible,deg,speed,gyro));}
        }
    }
    #[test]
    fn slow_but_continuous_neck_progress_is_not_a_stall() {
        let mut d=HeadGroundDetector::default();
        for i in 0..200 {
            let now=i as f64*0.02;
            let mut q=[0.0;15];q[5]=-1.4+0.1*now;
            let mut target=q;target[5]+=0.5;
            assert!(!update(&mut d,&q,Some(&target),now,true,30.0,0.1,0.0));
        }
    }

    #[test]
    fn interruption_clock_gap_and_invalid_feedback_restart_obstruction_timer() {
        let mut q=[0.0;15];q[5]=-1.4;let mut t=q;t[5]+=0.5;
        for (eligible,speed,gap) in [(false,0.0,0.02),(true,0.3,0.02),(true,0.0,0.3)] {
            let mut d=HeadGroundDetector::default();
            for i in 0..40 {assert!(!update(&mut d,&q,Some(&t),i as f64*0.02,true,30.0,0.0,0.0));}
            let restart=0.8+gap;
            assert!(!update(&mut d,&q,Some(&t),restart,eligible,30.0,speed,0.0));
            for i in 1..40 {assert!(!update(&mut d,&q,Some(&t),restart+i as f64*0.02,true,30.0,0.0,0.0));}
        }
        q[6]=f64::NAN;
        let mut d=HeadGroundDetector::default();
        assert!(!update(&mut d,&q,Some(&t),0.0,true,30.0,0.0,0.0));
        assert!(!d.update([f64::NAN,0.0,-1.0],&[0.0;15],&[0.0;15],[0.0;3],None,0.0,true));
    }
    #[test]
    fn no_contact_during_pick_or_roulade_even_in_folded_posture() {
        let mut q=[0.0;15];q[5]=-3.18;
        for rolling in [false,true] {
            let mut d=HeadGroundDetector::default();let mut f=Flow::default();
            if rolling {f.start_roulade(0.0).unwrap();} else {f.start_pick().unwrap();}
            for i in 0..50 {
                let now=i as f64*0.02;
                let eligible=f.phase==Phase::Walk && f.pick_phi.is_none() && !f.rolling();
                let contact=update(&mut d,&q,None,now,eligible,30.0,0.0,0.0);
                assert!(!contact);
                let frame=f.frame_with_head(gravity(30.0),now,true,[0.0;3],[0.0;4],0.0,contact).unwrap();
                assert_eq!(f.phase,Phase::Walk);f.inferred(&frame);
            }
        }
    }
}
