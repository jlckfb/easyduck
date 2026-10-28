// Easyduck migration 2026-10-21: zero file moved to /etc/robot.
// SCS/Feetech backend for the HD1910 servos.
use std::f64::consts::PI;
use std::io::{Read, Write};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::imu::ImuData;
use crate::imu_driver::I2cImuConfig;
use crate::imu_driver::{Reader, Calibration};
use crate::io::{ImuStale, IoError, JointTargets, Result, RobotIo, Sensors, SlowSensors};
use crate::model::{DEFAULT_POSITION, JOINT_IDS, NUM_JOINTS};

const BAUD: u32 = 1_000_000;
const READ_TIMEOUT: Duration = Duration::from_millis(15);
const POS: u8 = 56;
const BLOCK_LEN: u8 = 15; // addresses 56..=70; current is at 69/70
const TORQUE: u8 = 40;
const GAIN: u8 = 50;
const GOAL: u8 = 42;
const RAD_PER_COUNT: f64 = 2.0 * PI / 4096.0;
// Reference xgoduck Arduino: one speed register unit is 50 encoder counts/s.
const SCS_SPEED_RAD_PER_UNIT: f64 = 50.0 * RAD_PER_COUNT;
// Calibrated SCS/1910 zero and direction, in the robotd wire order [20..24,30..34,10..14].
const CAL_ZERO: [u16; NUM_JOINTS] = [994, 867, 2231, 2275, 359, 1358, 1756, 958, 235, 3960, 460, 1955, 2038, 1231, 3616];
const CAL_SIGN: [f64; NUM_JOINTS] = [-1.0; NUM_JOINTS];

const CALIBRATION_FILE: &str = "/etc/robot/zero_pos.json";

const VISUAL_CALIBRATION_FILE: &str = "/etc/robot/visual_calibration.json";

const JOINT_GAIN_FILE: &str = "/etc/robot/scs_joint_gains.json";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JointGainProfile {
    reference_gain: u16,
    joint_gains: BTreeMap<String, u16>,
}
fn parse_gain_profile(bytes: &[u8]) -> Result<JointGainProfile> {
    let profile: JointGainProfile = serde_json::from_slice(bytes)
        .map_err(|e| IoError::Bus(format!("invalid SCS joint gains: {e}")))?;
    if !(1..=255).contains(&profile.reference_gain) {
        return Err(IoError::Bus("SCS reference gain must be 1..255".into()));
    }
    for (id, gain) in &profile.joint_gains {
        if !JOINT_IDS.iter().any(|v| v.to_string() == *id) || !(1..=255).contains(gain) {
            return Err(IoError::Bus(format!("invalid SCS joint gain for ID {id}")));
        }
    }
    Ok(profile)
}
fn load_gain_profile(path: &Path) -> Result<Option<JointGainProfile>> {
    match std::fs::read(path) {
        Ok(bytes) => parse_gain_profile(&bytes).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(IoError::Bus(format!("cannot read SCS joint gains: {e}"))),
    }
}

#[derive(serde::Deserialize)]
struct SavedCalibration {
    zero: BTreeMap<String, u16>,
    sign: BTreeMap<String, i8>,
    lim: BTreeMap<String, [u16; 2]>,
}

#[derive(serde::Deserialize)]
struct VisualImu { mount: [f64; 4], gyro_bias: [f64; 3] }
#[derive(serde::Deserialize)]
struct VisualCalibration {
    #[serde(flatten)]
    joints: SavedCalibration,
    imu: VisualImu,
}
struct RuntimeCalibration {
    zero: [u16; NUM_JOINTS], sign: [f64; NUM_JOINTS],
    limits: [[u16; 2]; NUM_JOINTS], mount: [f64; 4], bias: [f64; 3],
}
fn parse_visual(bytes: &[u8]) -> Result<RuntimeCalibration> {
    let v: VisualCalibration = serde_json::from_slice(bytes)
        .map_err(|e| IoError::Bus(format!("invalid visual calibration: {e}")))?;
    let mut out = RuntimeCalibration {zero: [0; NUM_JOINTS], sign: [0.0; NUM_JOINTS],
        limits: [[0;2]; NUM_JOINTS], mount:v.imu.mount, bias:v.imu.gyro_bias};
    for (slot,id) in JOINT_IDS.iter().enumerate() {
        let key=id.to_string();
        let z=*v.joints.zero.get(&key).ok_or_else(||IoError::Bus(format!("missing zero {id}")))?;
        let sign=*v.joints.sign.get(&key).ok_or_else(||IoError::Bus(format!("missing sign {id}")))?;
        let lim=*v.joints.lim.get(&key).ok_or_else(||IoError::Bus(format!("missing limits {id}")))?;
        if ![-1,1].contains(&sign) || lim[0]>=lim[1] || lim[1]>4095 || !(lim[0]..=lim[1]).contains(&z) {
            return Err(IoError::Bus(format!("invalid visual joint calibration {id}")));
        }
        out.zero[slot]=z; out.sign[slot]=f64::from(sign); out.limits[slot]=lim;
    }
    let n:f64=out.mount.iter().map(|x|x*x).sum();
    if !n.is_finite() || (n-1.0).abs()>0.001 || out.bias.iter().any(|x|!x.is_finite() || x.abs()>0.2) {
        return Err(IoError::Bus("invalid visual IMU calibration".into()));
    }
    let n=n.sqrt(); for x in &mut out.mount {*x/=n;}
    Ok(out)
}
fn runtime_calibration(baseline: &Path) -> Result<RuntimeCalibration> {
    match std::fs::read(VISUAL_CALIBRATION_FILE) {
        Ok(bytes)=>parse_visual(&bytes),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(RuntimeCalibration {
            zero:CAL_ZERO,sign:CAL_SIGN,limits:load_limits(baseline)?,
            mount:crate::imu::SflpDecoder::DEFAULT_MOUNT,bias:[0.0;3]}),
        Err(e)=>Err(IoError::Bus(format!("cannot read visual calibration: {e}"))),
    }
}

fn parse_limits(bytes: &[u8]) -> Result<[[u16; 2]; NUM_JOINTS]> {
    let saved: SavedCalibration = serde_json::from_slice(bytes)
        .map_err(|e| IoError::Bus(format!("invalid SCS calibration: {e}")))?;
    let mut limits = [[0; 2]; NUM_JOINTS];
    for (slot, id) in JOINT_IDS.iter().enumerate() {
        let key = id.to_string();
        let bounds = saved.lim.get(&key)
            .ok_or_else(|| IoError::Bus(format!("SCS calibration has no limits for ID {id}")))?;
        let zero = saved.zero.get(&key)
            .ok_or_else(|| IoError::Bus(format!("SCS calibration has no zero for ID {id}")))?;
        let sign = saved.sign.get(&key)
            .ok_or_else(|| IoError::Bus(format!("SCS calibration has no sign for ID {id}")))?;
        if bounds[0] >= bounds[1] || bounds[1] > 4095 || !(bounds[0]..=bounds[1]).contains(zero) {
            return Err(IoError::Bus(format!("invalid SCS limits/zero for ID {id}")));
        }
        if *zero != CAL_ZERO[slot] || f64::from(*sign) != CAL_SIGN[slot] {
            return Err(IoError::Bus(format!("SCS zero/sign for ID {id} differs from the verified driver baseline")));
        }
        limits[slot] = *bounds;
    }
    Ok(limits)
}

fn load_limits(path: &Path) -> Result<[[u16; 2]; NUM_JOINTS]> {
    let bytes = std::fs::read(path)
        .map_err(|e| IoError::Bus(format!("cannot read SCS calibration {}: {e}", path.display())))?;
    parse_limits(&bytes)
}

pub struct ScsIo { port: Box<dyn SerialPort>, imu: Option<Reader>, last_imu: ImuData, imu_misses: (u64, u64), kd: u16, enable_gain: Option<u16>, armed: bool, cal_zero: [u16; NUM_JOINTS], cal_sign: [f64; NUM_JOINTS], limits: [[u16; 2]; NUM_JOINTS], gain_profile: Option<JointGainProfile>, turn_offsets: [i32;NUM_JOINTS] }

fn checksum(body: &[u8]) -> u8 { (!body.iter().map(|v| *v as u16).sum::<u16>()) as u8 }
fn frame(id: u8, instr: u8, params: &[u8]) -> Vec<u8> { let mut b = vec![id, (params.len()+2) as u8, instr]; b.extend_from_slice(params); let mut o=vec![0xff,0xff]; o.extend_from_slice(&b); o.push(checksum(&b)); o }
fn u16le(b: &[u8]) -> u16 { u16::from_le_bytes([b[0], b[1]]) }
// HLS positions use sign magnitude and may include complete turns.
fn signed_position(raw:u16)->i32 {
    let magnitude=i32::from(raw&0x7fff);
    if raw&0x8000!=0 {-magnitude} else {magnitude}
}
fn position_word(count:i32)->Result<u16> {
    if count.abs()>32767 {return Err(IoError::Bus("HLS target exceeds signed position range".into()));}
    Ok(count.unsigned_abs()as u16 | if count<0 {0x8000} else {0})
}
// Select the equivalent angle nearest the saved joint travel window. This also
// retains small negative readings at joints whose lower endpoint is zero.
fn logical_position(raw:u16, limits:[u16;2])->i32 {
    let count=signed_position(raw);
    let middle=(f64::from(limits[0])+f64::from(limits[1]))/2.0;
    let turns=((f64::from(count)-middle)/4096.0).round()as i32;
    count-turns*4096
}
fn current_ma(b:&[u8]) -> f64 { let raw=u16le(&b[13..15]); let sign=if raw&0x8000==0 {1.0} else {-1.0}; sign*f64::from(raw&0x7fff)*6.5 }

// Reject wrong-servo replies and malformed telemetry before it can become an arm target.
fn response_payload(req: &[u8], packet: &[u8], expected: usize) -> Result<Option<Vec<u8>>> {
    if packet==req {return Ok(None);} // Half-duplex echo is not a status reply.
    if packet.len()<6 || packet[..2]!=[0xff,0xff] {
        return Err(IoError::Bus("invalid SCS reply header".into()));
    }
    if packet[2]!=req[2] {return Ok(None);}
    if packet.len()!=4+packet[3] as usize || checksum(&packet[2..packet.len()-1])!=packet[packet.len()-1] {
        return Err(IoError::Bus("invalid SCS reply length/checksum".into()));
    }
    // Voltage-only status is advisory; retain every other alarm and packet check.
    if packet[4] & !0x01 != 0 {
        crate::io_trace::alarm(req[2],packet[4],req.get(5).copied(),packet);
        tracing::warn!(id=req[2],status=packet[4],overcurrent=packet[4]&8!=0,
            encoder=packet[4]&2!=0,overtemp=packet[4]&4!=0,
            packet=?packet,"validated SCS response alarm; not a register decode error");
        return Err(IoError::Bus(format!("SCS ID {} status error {}",req[2],packet[4])));
    }
    let actual=packet.len()-6;
    if actual==0 && expected>0 {return Ok(None);} // Late write ACK carries no sensor data.
    if actual!=expected {return Err(IoError::ShortRead{what:"SCS response",expected,got:actual});}
    Ok(Some(packet[5..packet.len()-1].to_vec()))
}

impl ScsIo {
    pub fn open(port: &str, cfg: Option<I2cImuConfig>) -> Result<Self> {
        Self::open_with_calibration(port, cfg, Path::new(CALIBRATION_FILE))
    }
    fn open_with_calibration(port: &str, cfg: Option<I2cImuConfig>, path: &Path) -> Result<Self> {
        // Reject an unusable calibration before opening either hardware device.
        let calibration = runtime_calibration(path)?;
        let limits = calibration.limits;
        let gain_profile = load_gain_profile(Path::new(JOINT_GAIN_FILE))?;
        let serial = serialport::new(port, BAUD).timeout(READ_TIMEOUT).open().map_err(|e| IoError::Port { path: port.into(), source: std::io::Error::other(e) })?;
        let imu = match cfg {
            Some(c)=>{
                let profile=match std::fs::read(VISUAL_CALIBRATION_FILE) {
                    Ok(bytes)=>serde_json::from_slice(&bytes).map_err(|e|IoError::Bus(e.to_string()))?,
                    Err(e) if e.kind()==std::io::ErrorKind::NotFound=>serde_json::Value::Null,
                    Err(e)=>return Err(IoError::Bus(e.to_string())),
                };
                let cal=Calibration::from_profile(&profile,&c.model).map_err(|e|IoError::Bus(e.to_string()))?;
                Some(Reader::start(c,cal).map_err(|e|IoError::Bus(e.to_string()))?)
            },None=>None
        };
        Ok(Self { cal_zero:calibration.zero, cal_sign:calibration.sign, port: serial, imu, last_imu: ImuData::default(), imu_misses:(0,0), kd:0, enable_gain:None, armed:false, limits, gain_profile, turn_offsets:[0;NUM_JOINTS] })
    }
    fn response(&mut self, req: &[u8], expected: usize) -> Result<Vec<u8>> {
        self.port.clear(serialport::ClearBuffer::All).ok();
        self.port.write_all(req).map_err(|e|IoError::Bus(e.to_string()))?;
        self.port.flush().map_err(|e|IoError::Bus(e.to_string()))?;
        let end=Instant::now()+READ_TIMEOUT;
        let mut buf=Vec::new(); let mut one=[0u8;128];
        while Instant::now()<end {
            match self.port.read(&mut one) {
                Ok(n) if n>0 => {
                    buf.extend_from_slice(&one[..n]);
                    loop {
                        let Some(start)=buf.windows(2).position(|w|w==[0xff,0xff]) else {
                            // Keep a possible first header byte across fragmented reads.
                            let tail=buf.last()==Some(&0xff); buf.clear();
                            if tail {buf.push(0xff);} break;
                        };
                        if start>0 {buf.drain(..start);}
                        if buf.len()<4 {break;}
                        let len=buf[3] as usize;
                        if len<2 {return Err(IoError::Bus("invalid SCS response length".into()));}
                        let total=4+len;
                        if buf.len()<total {break;}
                        let packet:Vec<u8>=buf.drain(..total).collect();
                        if let Some(payload)=response_payload(req,&packet,expected)? {return Ok(payload);}
                    }
                },
                Ok(_) => {},
                Err(e) if e.kind()==std::io::ErrorKind::TimedOut => {},
                Err(e) => return Err(IoError::Bus(e.to_string())),
            }
        }
        Err(IoError::Bus("SCS response timeout".into()))
    }
    fn read_reg(&mut self,id:u8,addr:u8,len:usize)->Result<Vec<u8>> { let body=[id,4,2,addr,len as u8]; let mut r=vec![0xff,0xff]; r.extend_from_slice(&body);r.push(checksum(&body));self.response(&r,len) }
    fn write_reg(&mut self,id:u8,addr:u8,data:&[u8])->Result<()> { let mut p=vec![addr];p.extend_from_slice(data);let r=frame(id,3,&p);self.port.clear(serialport::ClearBuffer::All).ok();self.port.write_all(&r).map_err(|e|IoError::Bus(e.to_string()))?;self.port.flush().map_err(|e|IoError::Bus(e.to_string()))?;
        // A PTY has no wire drain; let its firmware-emulator thread consume the write
        // before the next request clears the output queue. Production timing is unchanged.
        #[cfg(test)] std::thread::sleep(Duration::from_millis(2));
        std::thread::sleep(Duration::from_micros(300));Ok(()) }
    fn sync_write_positions(&mut self,t:&[f64;NUM_JOINTS])->Result<()> {
        let mut p=vec![GOAL,2];
        for(i,&id)in JOINT_IDS.iter().enumerate(){
            let count=self.cal_zero[i] as f64 + self.cal_sign[i]*(t[i]-DEFAULT_POSITION[i])/RAD_PER_COUNT;
            if !t[i].is_finite() || !count.is_finite() {
                return Err(IoError::Bus(format!("non-finite SCS target for ID {id}")));
            }
            let [lower,upper]=self.limits[i];
            let logical=count.round().clamp(f64::from(lower),f64::from(upper))as i32;
            let c=position_word(logical+self.turn_offsets[i])?;
            p.extend_from_slice(&[id,c as u8,(c>>8)as u8]);
        }
        // Construct and validate all joints before sending any bytes.
        let r=frame(0xfe,0x83,&p);
        let written=self.port.write_all(&r).and_then(|_|self.port.flush());
        crate::io_trace::wire(&p,t,written.is_ok());
        written.map_err(|e|IoError::Bus(e.to_string()))?;
        Ok(())
    }
    #[cfg(test)]
    fn decode(i:usize,b:&[u8],s:&mut Sensors)->Result<()>{Self::decode_cal(i,b,s,&CAL_ZERO,&CAL_SIGN,&parse_limits(include_bytes!("../tests/fixtures/scs_calibration_test_fixture.json")).unwrap())}
    fn decode_cal(i:usize,b:&[u8],s:&mut Sensors,zero:&[u16;NUM_JOINTS],sign:&[f64;NUM_JOINTS],limits:&[[u16;2];NUM_JOINTS])->Result<()> {if b.len()<BLOCK_LEN as usize{return Err(IoError::ShortRead{what:"SCS motor block",expected:BLOCK_LEN as usize,got:b.len()})}let p=f64::from(logical_position(u16le(&b[0..2]),limits[i]));let v=u16le(&b[2..4]);let mut vel=(v&0x7fff)as f64*SCS_SPEED_RAD_PER_UNIT;if v&0x8000!=0{vel=-vel}s.positions[i]=DEFAULT_POSITION[i] + sign[i] * (p - zero[i] as f64) * RAD_PER_COUNT;s.velocities[i]=vel;s.currents_ma[i]=current_ma(b);Ok(())}
    pub fn check_registers(&mut self)->Result<usize>{Ok(0)}
    pub fn set_position_id_gains(&mut self,_ki:u16,kd:u16){self.kd=kd}
    pub fn present_positions(&mut self)->Result<[f64;NUM_JOINTS]>{let mut o=[0.0;NUM_JOINTS];for(i,&id)in JOINT_IDS.iter().enumerate(){let b=self.read_reg(id,POS,2)?;o[i]=DEFAULT_POSITION[i] + self.cal_sign[i] * (f64::from(logical_position(u16le(&b),self.limits[i])) - self.cal_zero[i] as f64) * RAD_PER_COUNT;}Ok(o)}
    /// Cache configuration only: opening/restarting the daemon sends no motor writes.
    pub fn configure_enable_gain(&mut self, kp:u16) {self.enable_gain=Some(kp.min(255));}

    fn cut_all_torque(&mut self)->Result<()> {
        self.armed=false;
        let mut failed=Vec::new();
        for &id in &JOINT_IDS {
            let mut off=false;
            for _ in 0..3 {
                // A failure on one joint must not prevent emergency writes to the others.
                let written=self.write_reg(id,TORQUE,&[0]).is_ok();
                if written && self.read_reg(id,TORQUE,1).is_ok_and(|v|v==[0]) {off=true;break;}
            }
            if !off {failed.push(id);}
        }
        if failed.is_empty() {Ok(())} else {
            Err(IoError::Bus(format!("cannot verify torque off for IDs {failed:?}; disconnect servo power now")))
        }
    }
    fn fail_closed<T>(&mut self, cause:IoError)->Result<T> {
        match self.cut_all_torque() {
            Ok(()) => Err(cause),
            Err(off) => Err(IoError::Bus(format!("{cause}; {off}"))),
        }
    }
    fn checked_counts(&mut self)->Result<[u16;NUM_JOINTS]> {
        self.arm_counts(false)
    }
    fn arm_counts(&mut self, reference:bool)->Result<[u16;NUM_JOINTS]> {
        let mut counts=[0;NUM_JOINTS];
        for (slot,&id) in JOINT_IDS.iter().enumerate() {
            let count=u16le(&self.read_reg(id,POS,2)?);
            let [lo,hi]=self.limits[slot];
            let logical=logical_position(count,self.limits[slot]);
            if !reference && !(i32::from(lo)..=i32::from(hi)).contains(&logical) {
                return Err(IoError::Bus(format!("refusing to arm ID {id}: position {count} outside saved range {lo}..{hi}")));
            }
            counts[slot]=count;
        }
        Ok(counts)
    }
    fn running_gain_bytes(&self, slot: usize, requested: u16) -> [u8; 2] {
        let resolved = self.gain_profile.as_ref()
            .filter(|p| p.reference_gain == requested)
            .and_then(|p| p.joint_gains.get(&JOINT_IDS[slot].to_string()))
            .copied().unwrap_or(requested);
        [resolved.min(255) as u8, self.kd.min(255) as u8]
    }
    fn arm_at_current(&mut self)->Result<()> {
        self.arm_at_current_profile(false)
    }
    fn reset_head_yaw_turns(&mut self)->Result<()> {
        let raw=u16le(&self.read_reg(32,POS,2)?);
        let count=signed_position(raw);
        if (0..4096).contains(&count) {return Ok(());}
        // HD1910 mode 4 clips goals at the single-turn limit. Reset its volatile
        // turn counter while released; changing only the host coordinates can rotate a full turn.
        if ![0,4].contains(&self.read_reg(32,33,1)?[0])
            || self.read_reg(32,9,4)?!=[0,0,255,15] {
            return Err(IoError::Bus("ID 32 cannot reset turns with this position configuration".into()));
        }
        self.write_reg(32,TORQUE,&[0])?;
        if self.read_reg(32,TORQUE,1)?!=[0] {
            return Err(IoError::Bus("ID 32 torque-off verification failed before turn reset".into()));
        }
        self.response(&frame(32,0x0a,&[]),0)?;
        let after=signed_position(u16le(&self.read_reg(32,POS,2)?));
        if !(0..4096).contains(&after) || after.abs_diff(count.rem_euclid(4096))>8
            || self.read_reg(32,TORQUE,1)?!=[0] {
            return Err(IoError::Bus("ID 32 turn reset verification failed".into()));
        }
        self.turn_offsets[7]=0;
        Ok(())
    }
    fn arm_at_current_profile(&mut self, reference:bool)->Result<()> {
        let kp=self.enable_gain.ok_or_else(||IoError::Bus("SCS enable gain has not been configured".into()))?;
        self.reset_head_yaw_turns()?;
        let first=self.arm_counts(reference)?;
        let second=if reference {first} else {self.checked_counts()?};
        for i in 0..NUM_JOINTS {
            if !reference && signed_position(first[i]).abs_diff(signed_position(second[i]))>8 {
                return Err(IoError::Bus(format!("refusing to arm moving ID {}",JOINT_IDS[i])));
            }
        }
        // A goal write itself may enable torque. Write only the fresh current position,
        // without clamping, and verify each joint before touching the next one.
        for (i,&id) in JOINT_IDS.iter().enumerate() {
            let count=u16le(&self.read_reg(id,POS,2)?);
            let [lo,hi]=self.limits[i];
            let logical=logical_position(count,self.limits[i]);
            if !reference && (!(i32::from(lo)..=i32::from(hi)).contains(&logical)
                || signed_position(count).abs_diff(signed_position(second[i]))>8) {
                return Err(IoError::Bus(format!("ID {id} moved before arming")));
            }
            self.turn_offsets[i]=signed_position(count)-logical;
            self.write_reg(id,GOAL,&count.to_le_bytes())?;
            if u16le(&self.read_reg(id,GOAL,2)?)!=count
                || (!reference && signed_position(u16le(&self.read_reg(id,POS,2)?)).abs_diff(signed_position(count))>8) {
                return Err(IoError::Bus(format!("ID {id} current-goal verification failed")));
            }
        }
        // Residual targets have now been replaced; gain writes cannot act against boot goal 0.
        for (slot, &id) in JOINT_IDS.iter().enumerate() {
            let gains=self.running_gain_bytes(slot,kp);
            self.write_reg(id,GAIN,&gains)?;
            if self.read_reg(id,GAIN,2)?!=gains {return Err(IoError::Bus(format!("ID {id} gain verification failed")));}
            self.write_reg(id,TORQUE,&[1])?;
            if self.read_reg(id,TORQUE,1)?!=[1] {return Err(IoError::Bus(format!("ID {id} torque-on verification failed")));}
        }
        self.armed=true;
        Ok(())
    }
    pub fn set_torque(&mut self,on:bool)->Result<()> {
        let reference=std::env::var("ROBOTD_CONTROL_PROFILE").as_deref()==Ok("xgoduck");
        self.set_torque_profile(on,reference)
    }
    fn set_torque_profile(&mut self,on:bool,reference:bool)->Result<()> {
        if !on {return self.cut_all_torque();}
        self.armed=false;
        let result=if reference {self.arm_at_current_profile(true)} else {self.arm_at_current()};
        match result {Ok(())=>Ok(()),Err(e)=>self.fail_closed(e)}
    }
    pub fn interpolate_to(&mut self,t:&[f64;NUM_JOINTS],d:Duration,tick:Duration)->Result<()> {
        if !self.armed {return Err(IoError::Bus("SCS interpolation blocked while disarmed".into()));}
        if tick.is_zero() || t.iter().any(|v| !v.is_finite()) {
            return self.fail_closed(IoError::Bus("invalid SCS interpolation request".into()));
        }
        let start=match self.present_positions() {Ok(p)=>p,Err(e)=>return self.fail_closed(e)};
        let steps=(d.as_secs_f64()/tick.as_secs_f64()).ceil().max(1.0)as usize;
        for k in 1..=steps {
            let fraction=k as f64/steps as f64;
            let positions=std::array::from_fn(|i|start[i]+(t[i]-start[i])*fraction);
            self.write(&JointTargets::new(positions))?;
            std::thread::sleep(tick);
        }
        Ok(())
    }
}

impl RobotIo for ScsIo {
    fn target_ranges(&self) -> Option<[(f64,f64); NUM_JOINTS]> {
        Some(std::array::from_fn(|i| {
            let a=DEFAULT_POSITION[i]+self.cal_sign[i]*(self.limits[i][0] as f64-self.cal_zero[i] as f64)*RAD_PER_COUNT;
            let b=DEFAULT_POSITION[i]+self.cal_sign[i]*(self.limits[i][1] as f64-self.cal_zero[i] as f64)*RAD_PER_COUNT;
            (a.min(b),a.max(b))
        }))
    }

    fn read(&mut self)->Result<Sensors>{let mut s=Sensors::default();let mut blocks=[[0u8;15];15];for(i,&id)in JOINT_IDS.iter().enumerate(){let b=self.read_reg(id,POS,BLOCK_LEN as usize)?;Self::decode_cal(i,&b,&mut s,&self.cal_zero,&self.cal_sign,&self.limits)?;blocks[i].copy_from_slice(&b[..15]);}self.turn_offsets=std::array::from_fn(|i|{let raw=u16le(&blocks[i][..2]);signed_position(raw)-logical_position(raw,self.limits[i])});if let Some(i)=self.imu.as_ref(){match i.read_sample(){Some(v)=>{self.last_imu=v;self.imu_misses.1=0},None=>{self.imu_misses.0+=1;self.imu_misses.1+=1}}}s.imu=self.last_imu;crate::io_trace::feedback(&blocks);Ok(s)}
    fn write(&mut self,t:&JointTargets)->Result<()> {
        if !self.armed {return Err(IoError::Bus("SCS output blocked until explicit safe arm".into()));}
        match self.sync_write_positions(&t.positions) {Ok(())=>Ok(()),Err(e)=>self.fail_closed(e)}
    }
    fn set_torque(&mut self,on:bool)->Result<()>{ScsIo::set_torque(self,on)}
    fn set_gain(&mut self,kp:u16)->Result<()> {
        if !self.armed {return Err(IoError::Bus("SCS gain output blocked while disarmed".into()));}
        for (slot, &id) in JOINT_IDS.iter().enumerate() {
            let gains=self.running_gain_bytes(slot,kp);
            if let Err(e)=self.write_reg(id,GAIN,&gains) {return self.fail_closed(e);}
        }
        Ok(())
    }
    fn set_joint_gain(&mut self,j:usize,kp:u16)->Result<()> {
        if !self.armed {return Err(IoError::Bus("SCS joint gain blocked while disarmed".into()));}
        let id=*JOINT_IDS.get(j).ok_or_else(||IoError::Bus("joint index out of range".into()))?;
        match self.write_reg(id,GAIN,&[kp.min(255)as u8,self.kd.min(255)as u8]) {Ok(())=>Ok(()),Err(e)=>self.fail_closed(e)}
    }
    fn restore_joint_gain(&mut self,j:usize,kp:u16)->Result<()> {
        if !self.armed {return Err(IoError::Bus("SCS joint gain restore blocked while disarmed".into()));}
        let id=*JOINT_IDS.get(j).ok_or_else(||IoError::Bus("joint index out of range".into()))?;
        let gains=self.running_gain_bytes(j,kp);
        match self.write_reg(id,GAIN,&gains) {Ok(())=>Ok(()),Err(e)=>self.fail_closed(e)}
    }
    fn slow_sensors(&mut self)->Result<SlowSensors>{let mut v=Vec::new();let mut t=[0.0;NUM_JOINTS];for(i,&id)in JOINT_IDS.iter().enumerate(){let b=self.read_reg(id,62,2)?;v.push(b[0]as f64*0.1);t[i]=b[1]as f64}Ok(SlowSensors{volts:v.iter().sum::<f64>()/v.len()as f64,temps_c:t})}
    fn imu_stale(&self)->ImuStale{ImuStale{total:self.imu_misses.0,run:self.imu_misses.1}}
    fn imu_fresh(&self)->bool{self.imu.as_ref().is_some_and(|i|i.fresh())}
    fn imu_ready(&self)->bool{self.imu.as_ref().map(|i|i.ready()).unwrap_or(false)}
}

#[cfg(test)]
mod tests {
    #[test]
    fn visual_profile_joint_and_imu_are_validated() {
        let mut v:serde_json::Value=serde_json::from_slice(SAVED).unwrap();
        v["imu"]=serde_json::json!({"mount":[1,0,0,0],"gyro_bias":[0.01,-0.02,0.03]});
        v["zero"]["20"]=serde_json::json!(1000);v["sign"]["20"]=serde_json::json!(1);
        let c=parse_visual(&serde_json::to_vec(&v).unwrap()).unwrap();
        let mut b=[0u8;15];b[..2].copy_from_slice(&1128u16.to_le_bytes());
        let mut feedback=Sensors::default();ScsIo::decode_cal(0,&b,&mut feedback,&c.zero,&c.sign,&c.limits).unwrap();
        let expected=DEFAULT_POSITION[0]+128.0*RAD_PER_COUNT;
        assert!((feedback.positions[0]-expected).abs()<1e-12);
        let count=c.zero[0] as f64+c.sign[0]*(expected-DEFAULT_POSITION[0])/RAD_PER_COUNT;
        assert!((count-1128.0).abs()<1e-9);
        v["sign"]["20"]=serde_json::json!(0);
        assert!(parse_visual(&serde_json::to_vec(&v).unwrap()).is_err());
        v["sign"]["20"]=serde_json::json!(1);v["imu"]["mount"]=serde_json::json!([0,0,0,0]);
        assert!(parse_visual(&serde_json::to_vec(&v).unwrap()).is_err());
    }

    #[test]
    fn scs_reference_speed_units_decode_both_directions_and_zero() {
        for i in 0..NUM_JOINTS {
            for raw in [0u16, 1, 50, 0x8001, 0x8032, 0x8000] {
                let mut block = [0u8; BLOCK_LEN as usize];
                block[0..2].copy_from_slice(&CAL_ZERO[i].to_le_bytes());
                block[2..4].copy_from_slice(&raw.to_le_bytes());
                let mut sensors = Sensors::default();
                ScsIo::decode(i, &block, &mut sensors).unwrap();
                let direction = if raw & 0x8000 == 0 { 1.0 } else { -1.0 };
                let expected = f64::from(raw & 0x7fff) * direction * 50.0 * (2.0 * PI / 4096.0);
                assert!((sensors.velocities[i] - expected).abs() < 1e-12);
                assert!((sensors.positions[i] - DEFAULT_POSITION[i]).abs() < 1e-12);
            }
        }
    }

    use super::*;
    use std::fs::{File, OpenOptions};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const SAVED: &[u8] = include_bytes!("../tests/fixtures/scs_calibration_test_fixture.json");
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct CalibrationFile(std::path::PathBuf);
    impl CalibrationFile {
        fn new(bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("scs-limits-{}-{}-{}.json",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let mut file = OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
            file.write_all(bytes).unwrap();
            Self(path)
        }
    }
    impl Drop for CalibrationFile {
        fn drop(&mut self) { std::fs::remove_file(&self.0).unwrap(); }
    }

    fn pseudo_port() -> (String, File) {
        let (mut master, mut slave) = (-1, -1);
        let mut name = [0 as libc::c_char; 128];
        // A local pseudo-terminal captures the real serial writes; no robot is connected.
        assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, name.as_mut_ptr(),
            std::ptr::null(), std::ptr::null()) }, 0);
        let path = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_str().unwrap().to_owned();
        unsafe { libc::close(slave); }
        (path, unsafe { File::from_raw_fd(master) })
    }
    fn test_io(file: &CalibrationFile) -> (ScsIo, File) {
        let (port, master) = pseudo_port();
        {
            let mut io=ScsIo::open_with_calibration(&port,None,&file.0).unwrap();
            io.configure_enable_gain(6); io.set_position_id_gains(0,20); io.armed=true;
            (io,master)
        }
    }
    fn pending(master: &File) -> bool {
        let mut fd = libc::pollfd { fd: master.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        assert!(unsafe { libc::poll(&mut fd, 1, 20) } >= 0);
        fd.revents & libc::POLLIN != 0
    }
    fn packet_counts(master: &mut File) -> [u16; NUM_JOINTS] {
        assert!(pending(master), "expected a complete SCS goal packet");
        let mut packet = vec![0; 8 + 3 * NUM_JOINTS];
        master.read_exact(&mut packet).unwrap();
        assert_eq!(&packet[..7], &[0xff, 0xff, 0xfe, (4 + 3 * NUM_JOINTS) as u8, 0x83, GOAL, 2]);
        assert_eq!(*packet.last().unwrap(), checksum(&packet[2..packet.len()-1]));
        std::array::from_fn(|i| {
            let start = 7 + 3 * i;
            assert_eq!(packet[start], JOINT_IDS[i]);
            u16le(&packet[start+1..start+3])
        })
    }
    fn target_counts(counts: [f64; NUM_JOINTS]) -> [f64; NUM_JOINTS] {
        std::array::from_fn(|i| DEFAULT_POSITION[i]
            + CAL_SIGN[i] * (counts[i] - f64::from(CAL_ZERO[i])) * RAD_PER_COUNT)
    }

    #[test]
    fn scs_limits_saved_ids_are_matched_by_id_not_json_order() {
        let limits = parse_limits(SAVED).unwrap();
        let expected = [[0,1325],[562,1198],[875,4042],[1235,3420],[0,1113],
            [972,3393],[986,3096],[0,2501],[0,501],[3536,3964],
            [120,1225],[1590,2228],[428,3122],[2,2188],[2656,4095]];
        assert_eq!(limits, expected);
    }
    #[test]
    fn scs_limits_open_and_drop_do_not_send_servo_commands() {
        let file = CalibrationFile::new(SAVED);
        let (io, master) = test_io(&file);
        assert!(!pending(&master));
        drop(io);
        assert!(!pending(&master));
    }
    #[test]
    fn scs_limits_preserve_all_saved_stand_targets_on_the_wire() {
        let file = CalibrationFile::new(SAVED);
        let (mut io, mut master) = test_io(&file);
        io.write(&JointTargets::new(DEFAULT_POSITION)).unwrap();
        assert_eq!(packet_counts(&mut master), CAL_ZERO);
    }
    #[test]
    fn scs_limits_clamp_both_ends_of_all_fifteen_actual_packet_entries() {
        let file = CalibrationFile::new(SAVED);
        let (mut io, mut master) = test_io(&file);
        for (side, displacement) in [(0, -100_000.0), (1, 100_000.0)] {
            let wanted = target_counts(std::array::from_fn(|i| f64::from(io.limits[i][side]) + displacement));
            io.write(&JointTargets::new(wanted)).unwrap();
            assert_eq!(packet_counts(&mut master), std::array::from_fn(|i| io.limits[i][side]));
        }
    }
    #[test]
    fn scs_limits_interior_commands_keep_the_previous_conversion_and_rounding() {
        let file = CalibrationFile::new(SAVED);
        let (mut io, mut master) = test_io(&file);
        for fraction in [0.0, 0.001, 0.1, 0.25, 0.5, 0.75, 0.9, 0.999, 1.0] {
            let wanted = target_counts(std::array::from_fn(|i| {
                let [lo,hi] = io.limits[i]; f64::from(lo) + f64::from(hi-lo)*fraction
            }));
            let old = std::array::from_fn(|i| (f64::from(CAL_ZERO[i])
                + CAL_SIGN[i]*(wanted[i]-DEFAULT_POSITION[i])/RAD_PER_COUNT).round().clamp(0.0,4095.0) as u16);
            io.write(&JointTargets::new(wanted)).unwrap();
            assert_eq!(packet_counts(&mut master), old);
        }
    }
    #[test]
    fn scs_limits_bad_files_fail_before_hardware_open_without_fallback() {
        let original: serde_json::Value = serde_json::from_slice(SAVED).unwrap();
        let mut invalid = vec![b"not JSON".to_vec()];
        for (key, value) in [
            ("lim", serde_json::json!([1000,500])),
            ("lim", serde_json::json!([0,4096])),
            ("lim", serde_json::json!([995,1325])),
            ("lim", serde_json::json!([-1,1325])),
            ("lim", serde_json::json!([0,1325,2000])),
            ("lim", serde_json::json!([0,1325.5])),
            ("zero", serde_json::json!(995)),
            ("sign", serde_json::json!(1)),
            ("sign", serde_json::json!(0)),
        ] {
            let mut value_json = original.clone(); value_json[key]["20"] = value;
            invalid.push(serde_json::to_vec(&value_json).unwrap());
        }
        for section in ["zero", "sign", "lim"] {
            let mut value_json = original.clone();
            value_json[section].as_object_mut().unwrap().remove("34");
            invalid.push(serde_json::to_vec(&value_json).unwrap());
        }
        for bytes in invalid {
            let file = CalibrationFile::new(&bytes);
            assert!(matches!(ScsIo::open_with_calibration("/dev/should-not-be-opened", None, &file.0), Err(IoError::Bus(_))));
        }
        let missing = CalibrationFile::new(SAVED);
        std::fs::remove_file(&missing.0).unwrap();
        assert!(matches!(ScsIo::open_with_calibration("/dev/should-not-be-opened", None, &missing.0), Err(IoError::Bus(_))));
        std::mem::forget(missing);
        assert!(matches!(ScsIo::open_with_calibration("/dev/should-not-be-opened", None, Path::new("/tmp")), Err(IoError::Bus(_))));
    }
    #[test]
    fn scs_limits_non_finite_target_sends_no_partial_packet() {
        let file = CalibrationFile::new(SAVED);
        let (mut io, mut master) = test_io(&file);
        for slot in [0,7,14] {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
                let mut targets = DEFAULT_POSITION; targets[slot]=value;
                assert!(io.sync_write_positions(&targets).is_err());
                assert!(!pending(&master), "a partial packet escaped for slot {slot}");
            }
        }
        io.write(&JointTargets::new(DEFAULT_POSITION)).unwrap();
        assert_eq!(packet_counts(&mut master), CAL_ZERO);
    }
    #[test]
    fn scs_limits_file_changes_take_effect_only_when_reopened() {
        let file = CalibrationFile::new(SAVED);
        let (mut old_io, mut old_master) = test_io(&file);
        let mut value: serde_json::Value = serde_json::from_slice(SAVED).unwrap();
        value["lim"]["24"] = serde_json::json!([0,1000]);
        std::fs::write(&file.0, serde_json::to_vec(&value).unwrap()).unwrap();
        let wanted = target_counts(std::array::from_fn(|i| if JOINT_IDS[i]==24 {1383.0} else {f64::from(CAL_ZERO[i])}));
        old_io.write(&JointTargets::new(wanted)).unwrap();
        assert_eq!(packet_counts(&mut old_master)[4],1113);
        let (mut new_io, mut new_master) = test_io(&file);
        new_io.write(&JointTargets::new(wanted)).unwrap();
        assert_eq!(packet_counts(&mut new_master)[4],1000);
    }
    include!("scs_power_tests.rs");
    #[test]
    fn saved_ranges_preserve_current_left_hip_and_all_fifteen_wire_bounds() {
        let file=CalibrationFile::new(SAVED);
        let (mut io,mut master)=test_io(&file);
        let mut protection=crate::protection::Protection::new(Default::default());
        protection.set_target_ranges(io.target_ranges());
        let counts=[1049.0,587.0,3878.0,3322.0,359.0,1043.0,1718.0,821.0,268.0,3858.0,419.0,1737.0,1617.0,1440.0,3871.0];
        let mut q=target_counts(counts);
        assert_eq!(protection.guard(&mut q).clamped,0);
        io.write(&JointTargets::new(q)).unwrap();
        assert_eq!(packet_counts(&mut master),counts.map(|c|c as u16));
        for side in [0,1] {
            let mut q=target_counts(std::array::from_fn(|i|io.limits[i][side] as f64+if side==0{-100.0}else{100.0}));
            assert_eq!(protection.guard(&mut q).clamped,(1<<NUM_JOINTS)-1);
            io.write(&JointTargets::new(q)).unwrap();
            assert_eq!(packet_counts(&mut master),std::array::from_fn(|i|io.limits[i][side]));
        }
    }
}

#[cfg(test)]
mod voltage_status_tests {
    use super::*;
    #[test]
    fn only_voltage_bit_is_advisory() {
        let req=frame(12,2,&[56,2]);
        for status in 0u8..=255 {
            let packet=frame(12,status,&[0x34,0x02]);
            let result=response_payload(&req,&packet,2);
            if status & !1 == 0 {
                assert_eq!(result.unwrap(),Some(vec![0x34,0x02]));
            } else {
                assert!(result.is_err(),"status {status} must remain an error");
            }
        }
    }
    #[test]
    fn voltage_does_not_bypass_packet_validation() {
        let req=frame(12,2,&[56,2]);
        let mut bad=frame(12,1,&[0x34,0x02]);
        *bad.last_mut().unwrap()^=1;
        assert!(response_payload(&req,&bad,2).is_err());
        assert!(response_payload(&req,&frame(12,1,&[0x34]),2).is_err());
        assert_eq!(response_payload(&req,&frame(22,1,&[0x34,0x02]),2).unwrap(),None);
        assert_eq!(response_payload(&req,&frame(12,1,&[]),2).unwrap(),None);
    }
}


#[cfg(test)]
mod current_and_alarm_regression {
    use super::*;
    #[test]
    fn current_uses_both_bytes_69_70_and_sign_magnitude() {
        for (raw,wanted) in [(100u16,650.0),(0x8064,-650.0),(0x0123,1891.5)] {
            let mut b=[0u8;15];b[0..2].copy_from_slice(&CAL_ZERO[5].to_le_bytes());
            b[12]=0xff;b[13..15].copy_from_slice(&raw.to_le_bytes());
            let mut s=Sensors::default();ScsIo::decode(5,&b,&mut s).unwrap();
            assert_eq!(s.currents_ma[5],wanted);
        }
    }
    #[test]
    fn fourteen_bytes_cannot_supply_a_complete_current() {
        assert!(ScsIo::decode(5,&[0u8;14],&mut Sensors::default()).is_err());
    }
    #[test]
    fn alarm_8_is_response_status_not_register_data() {
        let req=frame(30,2,&[56,15]);
        let normal=frame(30,0,&[8u8;15]);
        assert!(response_payload(&req,&normal,15).is_ok());
        let alarm=frame(30,8,&[0u8;15]);
        assert!(response_payload(&req,&alarm,15).unwrap_err().to_string().contains("status error 8"));
        let mut corrupt=alarm;*corrupt.last_mut().unwrap()^=1;
        assert!(response_payload(&req,&corrupt,15).unwrap_err().to_string().contains("checksum"));
    }
}
