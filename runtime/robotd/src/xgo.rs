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
    q[5].is_finite() && q[6].is_finite() && q[5] <= -2.5 && (-0.8..=-0.1).contains(&q[6])
}
pub fn head_ground_condition(gravity:[f64;3], q:&[f64;15]) -> bool {
    let norm=gravity.iter().map(|v|v*v).sum::<f64>().sqrt();
    // Nominal 30 degrees, 2 degrees tolerance for the captured 29.2-degree resting pose.
    norm.is_finite() && norm>1e-6 && gravity[0]/norm>0.0
        && -gravity[2]/norm <= 28.0f64.to_radians().cos() && head_ground_posture(q)
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
                if angle > 45.0 || (angle >= 28.0 && gravity[0] > 0.0 && head_ground && self.pick_phi.is_none()) {
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
                if angle < 15.0 {
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
            for i in 0..20 { f.frame_with_head(gravity(deg),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,contact); }
            assert_eq!(f.phase,Phase::Walk);
        }
        let mut f=Flow::default();
        for i in 0..20 {f.frame_with_head(gravity(-30.0),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,true);}
        assert_eq!(f.phase,Phase::Walk);
    }
    #[test]
    fn short_dip_and_pick_do_not_become_a_head_ground_fall() {
        let mut f=Flow::default();
        for i in 0..7 {f.frame_with_head(gravity(30.0),i as f64*0.02,true,[0.0;3],[0.0;4],0.0,true);}
        f.frame_with_head(gravity(10.0),0.14,true,[0.0;3],[0.0;4],0.0,true);
        assert_eq!(f.phase,Phase::Walk);
        f.start_pick().unwrap();
        for i in 0..20 {f.frame_with_head(gravity(30.0),1.0+i as f64*0.02,true,[0.0;3],[0.0;4],0.0,true);}
        assert_eq!(f.phase,Phase::Walk);
    }
}
