// Firmware emulator on a local pseudo-terminal. No physical robot is connected.
#[derive(Clone)]
struct PowerState {
    counts:[u16;NUM_JOINTS], goals:[u16;NUM_JOINTS], torque:[u8;NUM_JOINTS],
    gains:[[u8;2];NUM_JOINTS], writes:Vec<(u8,u8,Vec<u8>)>, unsafe_enable:bool,
    reject_position:Option<u8>, bad_goal:Option<u8>, bad_gain:Option<u8>, stuck_torque:Option<u8>,
    moving:Option<u8>, position_reads:[usize;NUM_JOINTS],
}
impl PowerState {
    fn boot()->Self {
        Self {counts:CAL_ZERO, goals:[0;NUM_JOINTS],torque:[0;NUM_JOINTS],gains:[[32,40];NUM_JOINTS],
            writes:Vec::new(),unsafe_enable:false,reject_position:None,bad_goal:None,bad_gain:None,
            stuck_torque:None,moving:None,position_reads:[0;NUM_JOINTS]}
    }
}
struct PowerEmulator {
    state:std::sync::Arc<std::sync::Mutex<PowerState>>,
    stop:std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread:Option<std::thread::JoinHandle<()>>,
}
impl Drop for PowerEmulator {
    fn drop(&mut self) {
        self.stop.store(true,Ordering::Relaxed);
        if let Some(thread)=self.thread.take() {thread.join().unwrap();}
    }
}
fn power_emulator(initial:PowerState)->(ScsIo,PowerEmulator,CalibrationFile) {
    let file=CalibrationFile::new(SAVED);
    let (mut io,mut master)=test_io(&file); io.armed=false;
    let state=std::sync::Arc::new(std::sync::Mutex::new(initial));
    let stop=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shared=state.clone(); let stopping=stop.clone();
    let thread=std::thread::spawn(move || {
        while !stopping.load(Ordering::Relaxed) {
            let mut fd=libc::pollfd{fd:master.as_raw_fd(),events:libc::POLLIN,revents:0};
            if unsafe{libc::poll(&mut fd,1,5)}<=0 || fd.revents&libc::POLLIN==0 {continue;}
            let mut header=[0;4]; if master.read_exact(&mut header).is_err(){break;}
            assert_eq!(&header[..2],&[0xff,0xff]);
            let mut tail=vec![0;header[3] as usize]; if master.read_exact(&mut tail).is_err(){break;}
            let id=header[2]; let instruction=tail[0];
            let Some(slot)=JOINT_IDS.iter().position(|&v|v==id) else {panic!("unexpected broadcast in arm/off test")};
            let mut s=shared.lock().unwrap();
            if instruction==0x0a {
                assert_eq!(id,32);assert_eq!(s.torque[slot],0);
                s.writes.push((id,0x0a,Vec::new()));
                s.counts[slot]=(signed_position(s.counts[slot]).rem_euclid(4096))as u16;
                let reply=frame(id,0,&[]);drop(s);
                master.write_all(&reply).unwrap();continue;
            }
            let address=tail[1];
            if instruction==3 {
                let data=&tail[2..tail.len()-1]; s.writes.push((id,address,data.to_vec()));
                match address {
                    GOAL=>{s.goals[slot]=if id==32 {u16le(data).min(4096)} else {u16le(data)};s.torque[slot]=1;}, // Actual observed firmware behavior.
                    GAIN=>s.gains[slot]=[data[0],data[1]],
                    TORQUE=>{
                        if data[0]==1 && s.goals[slot]!=s.counts[slot] {s.unsafe_enable=true;}
                        s.torque[slot]=if s.stuck_torque==Some(id) {1} else {data[0]};
                    },
                    _=>panic!("unexpected servo write"),
                }
                continue;
            }
            assert_eq!(instruction,2);
            let n=tail[2] as usize;
            let mut error=0;
            let data=match address {
                POS=>{
                    s.position_reads[slot]+=1;
                    if s.reject_position==Some(id){error=2;}
                    let drift=if s.moving==Some(id) && s.position_reads[slot]>1 {20} else {0};
                    s.counts[slot]+=drift;
                    s.counts[slot].to_le_bytes().to_vec()
                },
                GOAL=>if s.bad_goal==Some(id){vec![0,0]}else{s.goals[slot].to_le_bytes().to_vec()},
                GAIN=>if s.bad_gain==Some(id){vec![32,40]}else{s.gains[slot].to_vec()},
                TORQUE=>vec![s.torque[slot]],
                33=>vec![4],
                9=>vec![0,0,255,15],
                _=>panic!("unexpected read address"),
            };
            assert_eq!(data.len(),n);
            let reply=frame(id,error,&data); drop(s);
            if master.write_all(&reply).is_err(){break;}
        }
    });
    (io,PowerEmulator{state,stop,thread:Some(thread)},file)
}
fn is_goal_or_on(w:&(u8,u8,Vec<u8>))->bool {w.1==GOAL || (w.1==TORQUE && w.2==[1])}

#[test]
fn scs_power_boot_goal_zero_is_replaced_before_torque_and_gain() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    io.set_torque(true).unwrap();
    let s=em.state.lock().unwrap();
    assert_eq!(s.goals,CAL_ZERO);assert_eq!(s.counts,CAL_ZERO);
    assert_eq!(s.torque,[1;NUM_JOINTS]);assert_eq!(s.gains,[[6,20];NUM_JOINTS]);assert!(!s.unsafe_enable);
    let first_gain=s.writes.iter().position(|w|w.1==GAIN).unwrap();
    assert_eq!(s.writes[..first_gain].iter().filter(|w|w.1==GOAL).count(),NUM_JOINTS);
}
#[test]
fn scs_power_outside_saved_limits_is_not_clamped_or_armed() {
    let mut state=PowerState::boot();state.counts[2]=874;
    let (mut io,em,_file)=power_emulator(state);assert!(io.set_torque(true).is_err());
    let s=em.state.lock().unwrap();assert!(!s.writes.iter().any(is_goal_or_on));assert_eq!(s.torque,[0;NUM_JOINTS]);
}
#[test]
fn scs_power_moving_position_refuses_arm() {
    let mut state=PowerState::boot();state.moving=Some(22);
    let (mut io,em,_file)=power_emulator(state);assert!(io.set_torque(true).is_err());
    assert!(!em.state.lock().unwrap().writes.iter().any(is_goal_or_on));
}

#[test]
fn xgo_power_gravity_drift_does_not_require_zero_following_error() {
    let mut state=PowerState::boot();state.moving=Some(22);
    let (mut io,em,_file)=power_emulator(state);
    io.gain_profile=Some(verified_gain_profile());
    io.set_torque_profile(true,true).unwrap();
    let s=em.state.lock().unwrap();
    assert_eq!(s.goals,s.counts);assert!(!s.unsafe_enable);
    assert_eq!(s.torque,[1;NUM_JOINTS]);
    assert_eq!(s.gains[1],[10,20]);
}
#[test]
fn xgo_power_outside_target_limit_is_armed_at_actual_position_without_clamping() {
    let mut state=PowerState::boot();state.counts[2]=874;
    let (mut io,em,_file)=power_emulator(state);
    io.set_torque_profile(true,true).unwrap();
    let s=em.state.lock().unwrap();assert_eq!(s.goals[2],874);assert!(!s.unsafe_enable);
}
#[test]
fn xgo_power_bad_reply_or_partial_write_failure_still_cuts_every_servo() {
    for fault in 0..2 {
        let mut state=PowerState::boot();
        if fault==0 {state.reject_position=Some(22);} else {state.bad_goal=Some(22);}
        let (mut io,em,_file)=power_emulator(state);
        assert!(io.set_torque_profile(true,true).is_err());
        assert_eq!(em.state.lock().unwrap().torque,[0;NUM_JOINTS]);assert!(!io.armed);
    }
}
#[test]
fn scs_power_bad_position_reply_fails_closed() {
    let mut state=PowerState::boot();state.reject_position=Some(22);
    let (mut io,em,_file)=power_emulator(state);assert!(io.set_torque(true).is_err());
    let s=em.state.lock().unwrap();assert!(!s.writes.iter().any(is_goal_or_on));assert_eq!(s.torque,[0;NUM_JOINTS]);
}
#[test]
fn scs_power_partial_arm_goal_failure_cuts_every_joint() {
    let mut state=PowerState::boot();state.bad_goal=Some(22);
    let (mut io,em,_file)=power_emulator(state);assert!(io.set_torque(true).is_err());
    let s=em.state.lock().unwrap();assert_eq!(s.torque,[0;NUM_JOINTS]);assert!(!io.armed);
    assert!(s.writes.iter().any(|w|w.1==GOAL));
    assert!(!s.writes.iter().any(|w|w.1==GOAL && u16le(&w.2)==0));
}
#[test]
fn scs_power_gain_verification_failure_cuts_every_joint() {
    let mut state=PowerState::boot();state.bad_gain=Some(22);
    let (mut io,em,_file)=power_emulator(state);assert!(io.set_torque(true).is_err());
    assert_eq!(em.state.lock().unwrap().torque,[0;NUM_JOINTS]);
}
#[test]
fn scs_power_off_failure_does_not_skip_other_joints() {
    let mut state=PowerState::boot();state.torque=[1;NUM_JOINTS];state.stuck_torque=Some(22);
    let (mut io,em,_file)=power_emulator(state);
    assert!(io.set_torque(false).unwrap_err().to_string().contains("disconnect servo power"));
    let s=em.state.lock().unwrap();
    for i in 0..NUM_JOINTS {assert_eq!(s.torque[i],if JOINT_IDS[i]==22 {1}else{0});}
    assert_eq!(s.writes.iter().filter(|w|w.0==22 && w.1==TORQUE).count(),3);
    assert!(!s.writes.iter().any(|w|w.1==GOAL || w.1==GAIN));
}
#[test]
fn scs_power_disarmed_target_and_gain_writes_are_blocked() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    io.set_torque(false).unwrap();let n=em.state.lock().unwrap().writes.len();
    assert!(io.write(&JointTargets::new(DEFAULT_POSITION)).is_err());
    assert!(io.set_gain(6).is_err());assert!(io.set_joint_gain(0,6).is_err());
    assert_eq!(em.state.lock().unwrap().writes.len(),n);
}
#[test]
fn scs_power_invalid_motion_disarms_and_requires_new_arm() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());io.set_torque(true).unwrap();
    let mut targets=DEFAULT_POSITION;targets[2]=f64::NAN;
    assert!(io.write(&JointTargets::new(targets)).is_err());
    assert_eq!(em.state.lock().unwrap().torque,[0;NUM_JOINTS]);assert!(!io.armed);
    assert!(io.write(&JointTargets::new(DEFAULT_POSITION)).is_err());
    io.set_torque(true).unwrap();assert!(io.armed);
}
#[test]
fn scs_power_reply_requires_correct_id_length_checksum_and_status() {
    let req=frame(22,2,&[POS,2]);let good=frame(22,0,&[0x34,0x12]);
    assert_eq!(response_payload(&req,&good,2).unwrap(),Some(vec![0x34,0x12]));
    assert_eq!(response_payload(&req,&frame(21,0,&[0,0]),2).unwrap(),None);
    assert_eq!(response_payload(&req,&req,2).unwrap(),None);
    assert_eq!(response_payload(&req,&frame(22,0,&[]),2).unwrap(),None);
    assert!(response_payload(&req,&frame(22,0,&[0]),2).is_err());
    assert!(response_payload(&req,&frame(22,2,&[0,0]),2).is_err());
    let mut corrupt=good;*corrupt.last_mut().unwrap()^=1;
    assert!(response_payload(&req,&corrupt,2).is_err());
}

#[test]
fn scs_standalone_interpolation_is_blocked_after_disarm() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    assert!(io.interpolate_to(&DEFAULT_POSITION,Duration::ZERO,Duration::from_millis(20)).is_err());
    assert!(em.state.lock().unwrap().writes.is_empty());
}
#[test]
fn scs_standalone_post_arm_read_failure_disarms_every_joint() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());io.set_torque(true).unwrap();
    em.state.lock().unwrap().reject_position=Some(22);
    assert!(io.interpolate_to(&DEFAULT_POSITION,Duration::ZERO,Duration::from_millis(20)).is_err());
    assert_eq!(em.state.lock().unwrap().torque,[0;NUM_JOINTS]);assert!(!io.armed);
}
#[test]
fn scs_standalone_invalid_target_disarms_without_restoring_goals() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());io.set_torque(true).unwrap();
    let mut targets=DEFAULT_POSITION;targets[2]=f64::NAN;
    assert!(io.interpolate_to(&targets,Duration::ZERO,Duration::from_millis(20)).is_err());
    let s=em.state.lock().unwrap();assert_eq!(s.torque,[0;NUM_JOINTS]);assert_eq!(s.goals,CAL_ZERO);assert!(!io.armed);
}

fn verified_gain_profile() -> JointGainProfile {
    parse_gain_profile(br#"{"reference_gain":6,"joint_gains":{"21":10,"24":10,"11":10}}"#).unwrap()
}
#[test]
fn scs_joint_profile_safe_arm_and_global_refresh_preserve_verified_gains() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    io.gain_profile=Some(verified_gain_profile());
    io.set_torque(true).unwrap();
    for phase in 0..2 {
        if phase==1 {io.set_gain(6).unwrap();}
        let s=em.state.lock().unwrap();
        assert!(!s.unsafe_enable);
        for (i,id) in JOINT_IDS.iter().enumerate() {
            assert_eq!(s.gains[i],[if [21,24,11].contains(id){10}else{6},20]);
        }
    }
    io.set_torque(false).unwrap();
    let count=em.state.lock().unwrap().writes.len();
    assert!(io.restore_joint_gain(1,6).is_err());
    assert!(io.set_gain(6).is_err());
    assert_eq!(em.state.lock().unwrap().writes.len(),count);
}
#[test]
fn scs_joint_profile_thermal_reduction_is_absolute_and_recovery_restores_profile() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    io.gain_profile=Some(verified_gain_profile());io.set_torque(true).unwrap();
    io.set_joint_gain(1,2).unwrap();
    assert_eq!(em.state.lock().unwrap().gains[1],[2,20]);
    io.restore_joint_gain(1,6).unwrap();
    assert_eq!(em.state.lock().unwrap().gains[1],[10,20]);
    io.set_gain(3).unwrap();
    assert!(em.state.lock().unwrap().gains.iter().all(|v|*v==[3,20]));
    io.set_torque(false).unwrap();
}
#[test]
fn scs_joint_profile_rejects_unknown_ids_and_invalid_gains() {
    for invalid in [
        br#"{"reference_gain":6,"joint_gains":{"99":10}}"#.as_slice(),
        br#"{"reference_gain":0,"joint_gains":{"21":10}}"#.as_slice(),
        br#"{"reference_gain":6,"joint_gains":{"21":256}}"#.as_slice(),
        br#"{"reference_gain":6,"joint_gains":{"21":0}}"#.as_slice(),
        br#"{"reference_gain":6,"joint_gains":{"021":10}}"#.as_slice(),
        br#"{"reference_gain":6,"joint_gains":{},"typo":10}"#.as_slice(),
    ] {assert!(parse_gain_profile(invalid).is_err());}
}
#[test]
fn scs_joint_profile_gain_readback_failure_still_cuts_every_servo() {
    let mut state=PowerState::boot();state.bad_gain=Some(24);
    let (mut io,em,_file)=power_emulator(state);
    io.gain_profile=Some(verified_gain_profile());
    assert!(io.set_torque(true).is_err());
    assert!(!io.armed);assert_eq!(em.state.lock().unwrap().torque,[0;NUM_JOINTS]);
}

#[test]
fn xgo_power_head_yaw_extra_turn_arms_without_revolving_or_recalibrating() {
    let mut state=PowerState::boot();state.counts[7]=4961;
    let (mut io,em,_file)=power_emulator(state);
    io.set_torque_profile(true,true).unwrap();
    let s=em.state.lock().unwrap();
    assert_eq!(s.goals[7],865);
    assert_eq!(s.counts[7],865);
    assert_eq!(s.writes.iter().filter(|(id,a,_)|*id==32 && *a==0x0a).count(),1);
    assert_eq!(io.turn_offsets[7],0);
    assert!(!s.unsafe_enable);
    drop(s);
    let q=io.present_positions().unwrap();
    let expected=DEFAULT_POSITION[7]+CAL_SIGN[7]*(865.0-CAL_ZERO[7]as f64)*RAD_PER_COUNT;
    assert!((q[7]-expected).abs()<1e-10);
}

#[test]
fn scs_turn_coordinates_keep_negative_near_zero_and_sign_magnitude() {
    assert_eq!(logical_position(0x800a,[0,1113]),-10);
    assert_eq!(logical_position(4961,[0,2501]),865);
    assert_eq!(logical_position(5402,[0,2501]),1306);
    for value in [-32767,-4096,-10,0,865,4961,32767] {
        assert_eq!(signed_position(position_word(value).unwrap()),value);
    }
    assert!(position_word(32768).is_err());
}
#[test]
fn scs_turn_wire_preserves_other_joint_turn_offsets() {
    let file=CalibrationFile::new(SAVED);
    let (mut io,mut master)=test_io(&file);
    io.turn_offsets[5]=4096;
    io.write(&JointTargets::new(DEFAULT_POSITION)).unwrap();
    let raw=packet_counts(&mut master);
    assert_eq!(raw[5],CAL_ZERO[5]+4096);
    for i in 0..NUM_JOINTS {if i!=5 {assert_eq!(raw[i],CAL_ZERO[i]);}}
}
#[test]
fn xgo_power_negative_ankle_arms_at_current_without_full_turn_goal() {
    let mut state=PowerState::boot();state.counts[4]=0x800a;
    let (mut io,em,_file)=power_emulator(state);
    io.set_torque_profile(true,true).unwrap();
    let s=em.state.lock().unwrap();
    assert_eq!(s.goals[4],0x800a);assert!(!s.unsafe_enable);
    assert_eq!(io.turn_offsets[4],0);
}

#[test]
fn xgo_power_single_turn_head_yaw_does_not_reset_any_servo() {
    let (mut io,em,_file)=power_emulator(PowerState::boot());
    io.set_torque_profile(true,true).unwrap();
    let s=em.state.lock().unwrap();
    assert!(!s.writes.iter().any(|(_,a,_)|*a==0x0a));
    assert_eq!(s.goals[7],CAL_ZERO[7]);
}
