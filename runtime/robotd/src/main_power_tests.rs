struct FailingPowerIo {
    inner:FakeIo, fail_arm:bool, fail_write:bool, arm_attempts:usize, write_attempts:usize,
}
impl RobotIo for FailingPowerIo {
    fn read(&mut self)->duck_control::io::Result<duck_control::Sensors>{self.inner.read()}
    fn set_gain(&mut self,kp:u16)->duck_control::io::Result<()>{self.inner.set_gain(kp)}
    fn set_joint_gain(&mut self,j:usize,kp:u16)->duck_control::io::Result<()>{self.inner.set_joint_gain(j,kp)}
    fn slow_sensors(&mut self)->duck_control::io::Result<duck_control::SlowSensors>{self.inner.slow_sensors()}
    fn set_torque(&mut self,on:bool)->duck_control::io::Result<()> {
        if on {self.arm_attempts+=1;}
        if on && self.fail_arm {
            self.inner.set_torque(false)?;
            return Err(duck_control::io::IoError::Bus("simulated verified arm failure".into()));
        }
        self.inner.set_torque(on)
    }
    fn write(&mut self,t:&duck_control::JointTargets)->duck_control::io::Result<()> {
        self.write_attempts+=1;
        if self.fail_write {
            // Match the SCS driver's fail-closed behavior, then test the service's reaction.
            self.inner.set_torque(false)?;
            return Err(duck_control::io::IoError::Bus("simulated output failure".into()));
        }
        self.inner.write(t)
    }
}
#[tokio::test]
async fn motor_output_failure_disables_request_and_never_rearms_itself() {
    let io=FailingPowerIo{inner:FakeIo::at(DEFAULT_POSITION).frozen(),fail_arm:false,fail_write:true,arm_attempts:0,write_attempts:0};
    let state=Arc::new(RobotState::new(&Params::default(),false,false));
    let intents=Arc::new(Intents::new());intents.set_enabled(true);intents.request_init(Some(Duration::from_millis(20)));
    let (tx,rx)=std::sync::mpsc::channel();let s=state.clone();let requests=intents.clone();
    let handle=tokio::spawn(async move {
        let mut io=io;control_loop_probe_with(&mut io,s,requests,Duration::from_millis(2)).await;
        tx.send((io.arm_attempts,io.write_attempts,io.inner.torque)).unwrap();
    });
    for _ in 0..200 {
        if state.ticks.load(Ordering::Relaxed)>=15 {break;}
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    state.shutdown.store(true,Ordering::Relaxed);handle.await.unwrap();
    assert_eq!(rx.recv().unwrap(),(1,1,Some(false)));
    assert!(!intents.snapshot().enabled);assert!(!state.homed.load(Ordering::Relaxed));
}
#[tokio::test]
async fn arm_failure_clears_enable_and_leaves_no_target_writes() {
    let io=FailingPowerIo{inner:FakeIo::at(DEFAULT_POSITION).frozen(),fail_arm:true,fail_write:false,arm_attempts:0,write_attempts:0};
    let state=Arc::new(RobotState::new(&Params::default(),false,false));
    let intents=Arc::new(Intents::new());intents.set_enabled(true);intents.request_init(None);
    let (tx,rx)=std::sync::mpsc::channel();let s=state.clone();let requests=intents.clone();
    let handle=tokio::spawn(async move {
        let mut io=io;control_loop_probe_with(&mut io,s,requests,Duration::from_millis(2)).await;
        tx.send((io.arm_attempts,io.write_attempts,io.inner.torque)).unwrap();
    });
    for _ in 0..200 {
        if state.ticks.load(Ordering::Relaxed)>=15 {break;}
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    state.shutdown.store(true,Ordering::Relaxed);handle.await.unwrap();
    assert_eq!(rx.recv().unwrap(),(1,0,Some(false)));assert!(!intents.snapshot().enabled);
}
