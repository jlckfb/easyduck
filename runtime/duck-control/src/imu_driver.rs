//! Sensor-frame contract and common processing. No device I/O in the control loop.
use crate::imu::{ImuData, mul, rotate, rotate_inverse};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Debug, Clone)]
pub struct I2cImuConfig {
    pub model: String,
    pub dev: String,
    pub addr: u8,
    pub fusion: String,
    pub lever_arm: [f64; 3],
    pub g_filter_k: f64,
    pub accel_trust_g: f64,
}
impl I2cImuConfig {
    pub fn validate(&self) -> io::Result<()> {
        let valid = match self.model.as_str() {
            "lsm6dsv16x" => {
                [0x6a, 0x6b].contains(&self.addr)
                    && ["sflp", "comp"].contains(&self.fusion.as_str())
            }
            "bno085" => [0x4a, 0x4b].contains(&self.addr) && self.fusion == "game_rv",
            _ => false,
        };
        if !valid
            || !self.dev.starts_with("/dev/i2c-")
            || !self.g_filter_k.is_finite()
            || !(0.0..=1.0).contains(&self.g_filter_k)
            || !self.accel_trust_g.is_finite()
            || self.accel_trust_g <= 0.0
            || self.lever_arm.iter().any(|v| !v.is_finite())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid IMU model/device/address/fusion or filter parameters",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Timed<const N: usize> {
    pub value: [f64; N],
    pub at: Instant,
}
/// Quaternion wxyz rotates sensor to world; gyro rad/s, accel m/s². None means missing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub quat: Option<Timed<4>>,
    pub gyro: Option<Timed<3>>,
    pub accel: Option<Timed<3>>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Capabilities {
    pub orientation: bool,
    pub gyro: bool,
    pub accel: bool,
}
pub trait Driver: Send {
    fn capabilities(&self) -> Capabilities;
    /// Only newly received reports. Returning a cached report here violates the contract.
    fn read(&mut self) -> io::Result<Sample>;
    fn close(&mut self) -> io::Result<()>;
}
pub fn open(c: &I2cImuConfig) -> io::Result<Box<dyn Driver>> {
    c.validate()?;
    match c.model.as_str() {
        "lsm6dsv16x" => Ok(Box::new(crate::imu_i2c::Lsm6dsv16xI2c::open(c)?)),
        "bno085" => Ok(Box::new(crate::imu_bno085::Bno085::open(c)?)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unregistered IMU",
        )),
    }
}
pub fn normalized(q: [f64; 4]) -> io::Result<[f64; 4]> {
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !n.is_finite() || !(1e-9..=10.0).contains(&n) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid quaternion",
        ));
    }
    Ok(q.map(|v| v / n))
}
#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    pub mount: [f64; 4],
    pub bias: [f64; 3],
    pub confirmed: bool,
}
impl Calibration {
    /// Untagged historical files are LSM sensor-frame calibrations only.
    pub fn from_profile(v: &serde_json::Value, model: &str) -> io::Result<Self> {
        if v.get("imu_calibrations").is_some_and(|m| !m.is_object()) {
            return Err(io::Error::other("invalid IMU calibration map"));
        }
        let entry = v
            .get("imu_calibrations")
            .and_then(|m| m.get(model))
            .or_else(|| {
                v.get("imu").filter(|e| {
                    e.get("model")
                        .and_then(|s| s.as_str())
                        .unwrap_or("lsm6dsv16x")
                        == model
                })
            });
        let Some(e) = entry else {
            return Ok(Self {
                mount: [1., 0., 0., 0.],
                bias: [0.; 3],
                confirmed: false,
            });
        };
        if e.get("model").is_some_and(|m| m.as_str() != Some(model)) {
            return Err(io::Error::other("IMU calibration model mismatch"));
        }
        if e.get("mount_valid").is_some_and(|v| !v.is_boolean()) {
            return Err(io::Error::other("invalid IMU calibration validity"));
        }
        let mount: [f64; 4] =
            serde_json::from_value(e["mount"].clone()).map_err(io::Error::other)?;
        let bias: [f64; 3] =
            serde_json::from_value(e["gyro_bias"].clone()).map_err(io::Error::other)?;
        if (mount.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 0.001
            || bias.iter().any(|v| !v.is_finite() || v.abs() > 0.2)
        {
            return Err(io::Error::other("invalid IMU calibration"));
        }
        if e.get("frame")
            .is_some_and(|f| f.as_str() != Some("sensor-v1"))
        {
            return Err(io::Error::other("unknown IMU calibration frame"));
        }
        Ok(Self {
            mount: normalized(mount)?,
            bias,
            confirmed: e
                .get("mount_valid")
                .and_then(|x| x.as_bool())
                .unwrap_or(true),
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub sample: Sample,
    pub caps: Capabilities,
    pub error: Option<String>,
    pub connected: bool,
}
const MAX_AGE: Duration = Duration::from_millis(150);
pub fn fresh<const N: usize>(v: Option<Timed<N>>, now: Instant) -> bool {
    v.is_some_and(|x| {
        now.checked_duration_since(x.at)
            .is_some_and(|age| age <= MAX_AGE)
    })
}
/// Apply mounting once, identically for every driver.
pub fn body(s: Sample, c: Calibration, now: Instant) -> Option<ImuData> {
    if !c.confirmed || !fresh(s.quat, now) || !fresh(s.gyro, now) {
        return None;
    }
    let q = normalized(mul(
        s.quat?.value,
        [c.mount[0], -c.mount[1], -c.mount[2], -c.mount[3]],
    ))
    .ok()?;
    let w = s.gyro?.value;
    let gyro = rotate(c.mount, std::array::from_fn(|i| w[i] - c.bias[i]));
    if gyro.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(ImuData {
        quat: q,
        gyro,
        gravity: rotate_inverse(q, [0., 0., -1.]),
    })
}
/// Host complementary fusion, sensor frame. Mounting and bias stay in this layer.
pub struct Processor {
    pub sample: Sample,
    config: I2cImuConfig,
    cal: Calibration,
    last: Option<Instant>,
    prev: [f64; 3],
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
impl Processor {
    pub fn new(config: I2cImuConfig, cal: Calibration) -> Self {
        Self {
            sample: Sample::default(),
            config,
            cal,
            last: None,
            prev: [0.; 3],
        }
    }
    pub fn update(&mut self, s: Sample) {
        if let Some(q) = s.quat {
            if let Ok(value) = normalized(q.value) {
                self.sample.quat = Some(Timed { value, ..q });
            }
        }
        if let Some(a) = s.accel {
            if a.value.iter().all(|v| v.is_finite()) {
                self.sample.accel = Some(a);
            }
        }
        if let Some(w) = s.gyro {
            if !w.value.iter().all(|v| v.is_finite()) {
                return;
            }
            self.sample.gyro = Some(w);
            if self.config.fusion != "comp" {
                return;
            }
            let Some(a) = self.sample.accel.filter(|a| fresh(Some(*a), w.at)) else {
                return;
            };
            let dt = self
                .last
                .map(|t| w.at.saturating_duration_since(t).as_secs_f64())
                .unwrap_or(0.);
            self.last = Some(w.at);
            let gyro = std::array::from_fn(|i| w.value[i] - self.cal.bias[i]);
            let r = rotate_inverse(self.cal.mount, self.config.lever_arm);
            let alpha = if dt > 0. && dt < 0.1 {
                std::array::from_fn(|i| (gyro[i] - self.prev[i]) / dt)
            } else {
                [0.; 3]
            };
            self.prev = gyro;
            let cent = cross(gyro, cross(gyro, r));
            let tang = cross(alpha, r);
            let a: [f64; 3] = std::array::from_fn(|i| a.value[i] - cent[i] - tang[i]);
            let mag = a.iter().map(|v| v * v).sum::<f64>().sqrt();
            let trusted = mag > 1e-6 && (mag / 9.80665 - 1.).abs() < self.config.accel_trust_g;
            if self.sample.quat.is_none() || dt >= 0.1 {
                // No invented initial orientation: align measured specific force with world up.
                self.sample.quat = None;
                if !trusted {
                    return;
                }
                let u = a.map(|v| v / mag);
                let q = if u[2] < -0.999999 {
                    [0., 1., 0., 0.]
                } else {
                    match normalized([1. + u[2], u[1], -u[0], 0.]) {
                        Ok(q) => q,
                        Err(_) => return,
                    }
                };
                self.sample.quat = Some(Timed { value: q, at: w.at });
                return;
            }
            if dt <= 0. {
                return;
            }
            let q = self.sample.quat.unwrap().value;
            let correction = if trusted {
                cross(a.map(|v| v / mag), rotate_inverse(q, [0., 0., 1.]))
            } else {
                [0.; 3]
            };
            let rate: [f64; 3] =
                std::array::from_fn(|i| gyro[i] + self.config.g_filter_k * correction[i] / dt);
            let dq = mul(q, [0., rate[0], rate[1], rate[2]]);
            if let Ok(value) = normalized(std::array::from_fn(|i| q[i] + 0.5 * dt * dq[i])) {
                self.sample.quat = Some(Timed { value, at: w.at });
            }
        }
    }
}
pub struct Reader {
    state: Arc<Mutex<Status>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    cal: Calibration,
}
impl Reader {
    pub fn start(c: I2cImuConfig, cal: Calibration) -> io::Result<Self> {
        c.validate()?;
        let state = Arc::new(Mutex::new(Status::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let quit = stop.clone();
        let worker = thread::Builder::new()
            .name("imu-reader".into())
            .spawn(move || {
                let mut d = match open(&c) {
                    Ok(d) => d,
                    Err(e) => {
                        if let Ok(mut s) = shared.lock() {
                            tracing::warn!(model=%c.model,error=%e,"IMU initialization failed");
                            s.error = Some(format!("{}: {e}", c.model));
                        }
                        return;
                    }
                };
                let mut p = Processor::new(c, cal);
                let caps = d.capabilities();
                let mut backoff = 20;
                let mut last_error = None;
                while !quit.load(Ordering::Acquire) {
                    let result = d.read();
                    let mut status = Status {
                        sample: p.sample,
                        caps,
                        error: None,
                        connected: true,
                    };
                    match result {
                        Ok(sample) => {
                            let any = sample.quat.is_some()
                                || sample.gyro.is_some()
                                || sample.accel.is_some();
                            p.update(sample);
                            status.sample = p.sample;
                            let now = Instant::now();
                            status.connected = fresh(p.sample.quat, now)
                                || fresh(p.sample.gyro, now)
                                || fresh(p.sample.accel, now);
                            backoff = if any { 20 } else { (backoff * 2).min(500) };
                        }
                        Err(e) => {
                            status.error = Some(e.to_string());
                            status.connected = false;
                            backoff = if e.kind() == io::ErrorKind::WouldBlock {
                                20
                            } else {
                                (backoff * 2).min(500)
                            };
                        }
                    }
                    if status.error != last_error {
                        if let Some(e) = &status.error {
                            tracing::warn!(error=%e,"IMU read failed");
                        }
                        last_error = status.error.clone();
                    }
                    if let Ok(mut s) = shared.lock() {
                        *s = status;
                    }
                    // park is interruptible by Drop; no device lock is held during I/O or waiting.
                    thread::park_timeout(Duration::from_millis(backoff));
                }
                let result = d.close();
                if let Ok(mut s) = shared.lock() {
                    s.connected = false;
                    if let Err(e) = result {
                        s.error = Some(e.to_string());
                    }
                }
            })?;
        Ok(Self {
            state,
            stop,
            worker: Some(worker),
            cal,
        })
    }
    pub fn status(&self) -> Option<Status> {
        self.state.try_lock().ok().map(|s| s.clone())
    }
    pub fn read_sample(&self) -> Option<ImuData> {
        let s = self.status()?;
        if s.error.is_some() {
            return None;
        }
        body(s.sample, self.cal, Instant::now())
    }
    pub fn fresh(&self) -> bool {
        self.read_sample().is_some()
    }
    pub fn ready(&self) -> bool {
        self.fresh()
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.worker.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vec<const N: usize>(v: &serde_json::Value) -> [f64; N] {
        std::array::from_fn(|i| v[i].as_f64().unwrap())
    }
    fn near(a: &[f64], b: &[f64]) {
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 1e-8, "{a:?} != {b:?}");
        }
    }
    fn config(mode: &str) -> I2cImuConfig {
        I2cImuConfig {
            model: "lsm6dsv16x".into(),
            dev: "/dev/i2c-2".into(),
            addr: 0x6b,
            fusion: mode.into(),
            lever_arm: [0.; 3],
            g_filter_k: 0.05,
            accel_trust_g: 0.3,
        }
    }
    #[test]
    fn shared_contract_vectors() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("imu_contract_vectors.json")).unwrap();
        let now = Instant::now();
        for v in fixtures["transforms"].as_array().unwrap() {
            let cal = Calibration {
                mount: vec(&v["mount"]),
                bias: vec(&v["bias"]),
                confirmed: true,
            };
            let s = Sample {
                quat: Some(Timed {
                    value: vec(&v["quat"]),
                    at: now,
                }),
                gyro: Some(Timed {
                    value: vec(&v["gyro"]),
                    at: now,
                }),
                accel: None,
            };
            let b = body(s, cal, now).unwrap();
            near(&b.quat, &vec::<4>(&v["body_quat"]));
            near(&b.gyro, &vec::<3>(&v["body_gyro"]));
            near(&b.gravity, &vec::<3>(&v["gravity"]));
        }
    }
    #[test]
    fn timestamps_missing_stale_and_unconfirmed() {
        let now = Instant::now();
        let cal = Calibration {
            mount: [1., 0., 0., 0.],
            bias: [0.; 3],
            confirmed: true,
        };
        let mut s = Sample {
            quat: Some(Timed {
                value: [1., 0., 0., 0.],
                at: now,
            }),
            gyro: None,
            accel: None,
        };
        assert!(body(s, cal, now).is_none());
        s.gyro = Some(Timed {
            value: [0.; 3],
            at: now - Duration::from_millis(151),
        });
        assert!(body(s, cal, now).is_none());
        s.gyro.as_mut().unwrap().at = now;
        assert!(body(s, cal, now).is_some());
        assert!(body(s, cal, now + Duration::from_millis(151)).is_none());
        assert!(
            body(
                s,
                Calibration {
                    confirmed: false,
                    ..cal
                },
                now
            )
            .is_none()
        );
        assert!(!fresh(s.quat, now - Duration::from_millis(1)));
        let mut p = Processor::new(config("sflp"), cal);
        p.update(s);
        p.update(Sample {
            gyro: Some(Timed {
                value: [1.; 3],
                at: now + Duration::from_millis(200),
            }),
            ..Sample::default()
        });
        assert_eq!(p.sample.quat.unwrap().at, now);
    }
    #[test]
    fn calibration_model_isolation_and_legacy() {
        let old = serde_json::json!({"imu":{"mount":[1,0,0,0],"gyro_bias":[0,0,0]}});
        assert!(
            Calibration::from_profile(&old, "lsm6dsv16x")
                .unwrap()
                .confirmed
        );
        assert!(!Calibration::from_profile(&old, "bno085").unwrap().confirmed);
        let tagged = serde_json::json!({"imu":{"model":"bno085","frame":"wrong","mount":[1,0,0,0],"gyro_bias":[0,0,0]}});
        assert!(Calibration::from_profile(&tagged, "bno085").is_err());
    }
    #[test]
    fn complementary_uses_measured_tilt_and_si_units() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("imu_contract_vectors.json")).unwrap();
        let v = &fixtures["comp"];
        let now = Instant::now();
        let cal = Calibration {
            mount: [1., 0., 0., 0.],
            bias: [0.; 3],
            confirmed: true,
        };
        let mut p = Processor::new(config("comp"), cal);
        assert!(p.sample.quat.is_none());
        p.update(Sample {
            gyro: Some(Timed {
                value: vec(&v["gyro"]),
                at: now,
            }),
            accel: Some(Timed {
                value: vec(&v["accel"]),
                at: now,
            }),
            quat: None,
        });
        near(&p.sample.quat.unwrap().value, &vec::<4>(&v["quat"]));
        near(
            &body(p.sample, cal, now).unwrap().gravity,
            &vec::<3>(&v["gravity"]),
        );
        let at = now + Duration::from_secs(1);
        p.update(Sample {
            gyro: Some(Timed { value: [0.; 3], at }),
            ..Sample::default()
        });
        assert_eq!(p.sample.quat.unwrap().at, now);
    }
    #[test]
    fn bad_model_or_fusion_is_rejected_before_io() {
        let mut c = config("game_rv");
        assert!(c.validate().is_err());
        c.model = "bno085".into();
        c.addr = 0x4b;
        assert!(c.validate().is_ok());
        c.fusion = "sflp".into();
        assert!(c.validate().is_err());
    }
    #[test]
    fn reader_failure_closes_and_does_not_report_ready() {
        let mut c = config("sflp");
        c.dev = "/dev/i2c-no-such-device".into();
        let reader = Reader::start(
            c,
            Calibration {
                mount: [1., 0., 0., 0.],
                bias: [0.; 3],
                confirmed: true,
            },
        )
        .unwrap();
        for _ in 0..50 {
            if reader.status().is_some_and(|s| s.error.is_some()) {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!reader.ready());
        assert!(reader.status().unwrap().error.is_some());
        drop(reader);
    }
}
