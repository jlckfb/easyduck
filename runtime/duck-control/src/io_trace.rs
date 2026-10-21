//! Passive, bounded capture. A full queue or a file error never changes control results.
use std::{cell::RefCell, fs::OpenOptions, io::Write, sync::{OnceLock, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{SyncSender, sync_channel}}, time::Instant};
use crate::{Sensors, model::{JOINT_IDS, NUM_JOINTS}};

const MAX_BYTES: u64 = 128 * 1024 * 1024;
#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct RawFeedback {
    frame: u64,
    counts: [u16; NUM_JOINTS],
    speed: [u16; NUM_JOINTS],
    load: [u16; NUM_JOINTS],
    current: [u16; NUM_JOINTS],
    voltage: [u8; NUM_JOINTS],
    temperature: [u8; NUM_JOINTS],
}
#[derive(Default)]
struct Context { frame: u64, fresh: bool, raw: Option<RawFeedback> }
thread_local! { static CONTEXT: RefCell<Context> = RefCell::new(Context::default()); }
enum Data {
    Inference { input: Option<[f32; 61]>, action: [f32; 14], positions: [f64; 15], velocities: [f64; 15], gyro: [f64; 3], gravity: [f64; 3], quat: [f64; 4], targets: [f64; 15], network: &'static str, reset: bool },
    Alarm { id:u8, status:u8, address:Option<u8>, packet:Vec<u8> },
    Feedback { positions:[f64;15], current_ma:[f64;15], gravity:[f64;3], gyro:[f64;3] },
    Wire { payload: [u8; 47], targets: [f64; 15], success: bool },
}
struct Event { frame: u64, time: f64, fresh: bool, raw: Option<RawFeedback>, dropped: u64, data: Data }
struct Recorder { tx: SyncSender<Event>, enabled: AtomicBool, dropped: AtomicU64, start: Instant }
static RECORDER: OnceLock<Option<Recorder>> = OnceLock::new();

pub fn init_from_env() {
    RECORDER.get_or_init(|| {
        let base=std::env::var_os("ROBOTD_IO_TRACE")?;
        // Each daemon start gets its own file; preserve earlier evidence on restart.
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let path=std::path::PathBuf::from(base).with_extension(format!("{stamp}-{}.jsonl",std::process::id()));
        let mut file=match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(v)=>v, Err(e)=>{tracing::warn!(error=%e,"passive I/O capture unavailable");return None}
        };
        let (tx,rx)=sync_channel::<Event>(256);
        let worker=std::thread::Builder::new().name("robot-io-capture".into()).spawn(move || {
            let header=serde_json::json!({"event":"header","schema":2,"pid":std::process::id(),"unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),"joint_ids":JOINT_IDS,"max_bytes":MAX_BYTES});
            if writeln!(file,"{header}").is_err() { return; }
            let mut bytes=0u64;
            while let Ok(e)=rx.recv() {
                let data=match e.data {
                    Data::Inference{input,action,positions,velocities,gyro,gravity,quat,targets,network,reset}=>serde_json::json!({"event":"inference","input":input.as_ref().map(|a|a.as_slice()),"action":action,"positions":positions,"velocities":velocities,"gyro":gyro,"gravity":gravity,"quat":quat,"filtered_targets":targets,"network":network,"reset":reset}),
                    Data::Alarm{id,status,address,packet}=>serde_json::json!({"event":"servo_alarm","id":id,"status":status,"address":address,"packet":packet}),
                    Data::Feedback{positions,current_ma,gravity,gyro}=>serde_json::json!({"event":"feedback","positions":positions,"current_ma":current_ma,"gravity":gravity,"gyro":gyro}),
                    Data::Wire{payload,targets,success}=>serde_json::json!({"event":"wire","payload":payload.as_slice(),"requested_targets":targets,"write_success":success}),
                };
                let mut row=data;
                row["frame"]=e.frame.into();row["time_s"]=e.time.into();row["fresh"]=e.fresh.into();row["raw_feedback"]=serde_json::to_value(e.raw).unwrap_or_default();row["dropped_total"]=e.dropped.into();
                let mut line=match serde_json::to_vec(&row) {Ok(v)=>v,Err(_)=>continue};line.push(b'\n');
                bytes+=line.len() as u64;
                if let Err(err)=file.write_all(&line) {tracing::warn!(error=%err,"passive I/O capture write failed");break;}
                if bytes>=MAX_BYTES {let _=writeln!(file,"{{\"event\":\"capture_end\",\"reason\":\"size_limit\"}}");break;}
            }
            let _=file.flush();
            if let Some(Some(r))=RECORDER.get() {r.enabled.store(false,Ordering::Relaxed);}
        });
        if let Err(e)=worker {tracing::warn!(error=%e,"passive I/O capture thread unavailable");return None;}
        tracing::info!(path=?path,"passive I/O capture ready; no motor/control changes");
        Some(Recorder{tx,enabled:AtomicBool::new(true),dropped:AtomicU64::new(0),start:Instant::now()})
    });
}
fn recorder()->Option<&'static Recorder> {RECORDER.get().and_then(|v|v.as_ref()).filter(|r|r.enabled.load(Ordering::Relaxed))}
pub fn begin_frame() {if recorder().is_some() {CONTEXT.with(|c|{let mut c=c.borrow_mut();c.frame+=1;c.fresh=false;});}}
pub fn feedback_status(fresh:bool) {if recorder().is_some() {CONTEXT.with(|c|c.borrow_mut().fresh=fresh);}}
pub fn feedback(blocks:&[[u8;15];15]) {
    if recorder().is_none() {return;}
    CONTEXT.with(|c|{let mut c=c.borrow_mut();let mut raw=RawFeedback{frame:c.frame,..RawFeedback::default()};
        for (i,b) in blocks.iter().enumerate() {raw.counts[i]=u16::from_le_bytes([b[0],b[1]]);raw.speed[i]=u16::from_le_bytes([b[2],b[3]]);raw.load[i]=u16::from_le_bytes([b[4],b[5]]);raw.voltage[i]=b[6];raw.temperature[i]=b[7];raw.current[i]=u16::from_le_bytes([b[13],b[14]]);}
        c.raw=Some(raw);
    });
}
fn send(data:Data) {
    let Some(r)=recorder() else {return};
    let event=CONTEXT.with(|c|{let c=c.borrow();Event{frame:c.frame,time:r.start.elapsed().as_secs_f64(),fresh:c.fresh,raw:c.raw,dropped:r.dropped.load(Ordering::Relaxed),data}});
    match r.tx.try_send(event) {Ok(())=>(),Err(std::sync::mpsc::TrySendError::Full(_))=>{r.dropped.fetch_add(1,Ordering::Relaxed);},Err(std::sync::mpsc::TrySendError::Disconnected(_))=>r.enabled.store(false,Ordering::Relaxed)}
}
pub fn inference(input:Option<&[f32]>, action:&[f32;14], sensors:&Sensors, targets:&[f64;15], network:&'static str, reset:bool) {
    if recorder().is_none() {return;}
    let input=input.map(|s|{let mut a=[0.;61];a.copy_from_slice(s);a});
    send(Data::Inference{input,action:*action,positions:sensors.positions,velocities:sensors.velocities,gyro:sensors.imu.gyro,gravity:sensors.imu.gravity,quat:sensors.imu.quat,targets:*targets,network,reset});
}
pub fn wire(payload:&[u8], targets:&[f64;15], success:bool) {
    if recorder().is_none() || payload.len()!=47 {return;}
    let mut p=[0;47];p.copy_from_slice(payload);send(Data::Wire{payload:p,targets:*targets,success});
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturated_queue_returns_without_waiting() {
        let (tx,_rx)=sync_channel(1);
        tx.try_send(1).unwrap();
        assert!(matches!(tx.try_send(2),Err(std::sync::mpsc::TrySendError::Full(2))));
    }
    #[test]
    fn packet_payload_has_fifteen_id_count_triples() {
        assert_eq!(47,2+JOINT_IDS.len()*3);
    }
}

pub fn alarm(id:u8,status:u8,address:Option<u8>,packet:&[u8]) {
    if recorder().is_some() { send(Data::Alarm{id,status,address,packet:packet.to_vec()}); }
}
pub fn sensors(s:&Sensors) {
    if recorder().is_some() { send(Data::Feedback{positions:s.positions,current_ma:s.currents_ma,gravity:s.imu.gravity,gyro:s.imu.gyro}); }
}
