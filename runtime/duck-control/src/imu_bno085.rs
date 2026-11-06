//! BNO085 SHTP/SH-2, independently implemented from CEVA's published protocol.
//! See docs/en/imu-drivers.md for references and the polling timestamp limitation.
use crate::{
    imu_driver::{Capabilities, Driver, I2cImuConfig, Sample, Timed, normalized},
    imu_transport::I2c,
};
use std::{
    io,
    time::{Duration, Instant},
};
fn bad(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s)
}
pub struct Reports {
    seq: [Option<u8>; 256],
}
impl Default for Reports {
    fn default() -> Self {
        Self { seq: [None; 256] }
    }
}
impl Reports {
    pub fn parse(&mut self, p: &[u8], received: Instant) -> io::Result<Sample> {
        let mut next = self.seq;
        let mut out = Sample::default();
        let mut i = 0;
        let mut reference = 0i64;
        while i < p.len() {
            let id = p[i];
            let n = match id {
                0xfb | 0xfa => 5,
                0xef => 2,
                1 | 2 => 10,
                8 => 12,
                _ => return Err(bad("unknown SH-2 sensor report")),
            };
            if i + n > p.len() {
                return Err(bad("truncated SH-2 report"));
            }
            let v = &p[i..i + n];
            i += n;
            if id == 0xfb {
                reference = -i64::from(u32::from_le_bytes(v[1..5].try_into().unwrap()));
                continue;
            }
            if id == 0xfa {
                reference += i64::from(i32::from_le_bytes(v[1..5].try_into().unwrap()));
                continue;
            }
            if id == 0xef {
                continue;
            }
            let delay = i64::from((u16::from(v[2] & 0xfc) << 6) + u16::from(v[3]));
            let offset = (reference + delay) * 100;
            if offset > 0 {
                return Err(bad("SH-2 timestamp is in the future"));
            }
            let at = received
                .checked_sub(Duration::from_micros(offset.unsigned_abs()))
                .ok_or_else(|| bad("SH-2 timestamp underflow"))?;
            let duplicate = next[id as usize].is_some_and(|old| {
                let d = v[1].wrapping_sub(old);
                d == 0 || d > 127
            });
            if duplicate {
                continue;
            }
            next[id as usize] = Some(v[1]);
            if v[2] & 3 == 0 {
                continue;
            }
            let scalar = |j| f64::from(i16::from_le_bytes([v[j], v[j + 1]]));
            match id {
                8 => {
                    let q = [
                        scalar(10) / 16384.,
                        scalar(4) / 16384.,
                        scalar(6) / 16384.,
                        scalar(8) / 16384.,
                    ];
                    if (q.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 0.02 {
                        return Err(bad("invalid BNO quaternion norm"));
                    }
                    out.quat = Some(Timed {
                        value: normalized(q)?,
                        at,
                    });
                }
                2 => {
                    out.gyro = Some(Timed {
                        value: [scalar(4) / 512., scalar(6) / 512., scalar(8) / 512.],
                        at,
                    })
                }
                1 => {
                    out.accel = Some(Timed {
                        value: [scalar(4) / 256., scalar(6) / 256., scalar(8) / 256.],
                        at,
                    })
                }
                _ => {}
            }
        }
        self.seq = next;
        Ok(out)
    }
}
// A four-byte read consumes a header-only transfer. The remaining cargo can
// carry a continuation header with the next sequence; some HALs repeat it.
fn complete_transfer(h: &[u8], p: &[u8]) -> io::Result<u8> {
    if h.len() != 4 || p.len() < 4 {
        return Err(bad("short SHTP transfer"));
    }
    let raw = u16::from_le_bytes([h[0], h[1]]);
    let tail = u16::from_le_bytes([p[0], p[1]]);
    if raw & 0x8000 != 0 || p.len() != usize::from(raw) {
        return Err(bad("orphan or truncated SHTP continuation"));
    }
    if p[..4] != *h && !(tail == (raw | 0x8000) && p[2] == h[2] && p[3] == h[3].wrapping_add(1)) {
        return Err(bad("SHTP continuation does not match header"));
    }
    Ok(p[3])
}
pub struct Bno085 {
    bus: I2c,
    tx: [u8; 6],
    rx: [Option<u8>; 6],
    reports: Reports,
    caps: Capabilities,
    features: [Option<bool>; 3],
}
impl Bno085 {
    pub fn open(c: &I2cImuConfig) -> io::Result<Self> {
        let mut d = Self {
            bus: I2c::open(&c.dev, c.addr)?,
            tx: [0; 6],
            rx: [None; 6],
            reports: Reports::default(),
            caps: Capabilities::default(),
            features: [None; 3],
        };
        d.send(2, &[0xf9, 0])?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut product = false;
        while Instant::now() < deadline {
            if let Some((ch, p)) = d.packet()? {
                if ch == 2 && p.first() == Some(&0xf8) {
                    if p.len() % 16 != 0 || p.chunks_exact(16).any(|report| report[0] != 0xf8) {
                        return Err(bad("truncated Product ID"));
                    }
                    product = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !product {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "BNO085 Product ID timeout",
            ));
        }
        for id in [8, 2, 1] {
            let mut p = [0u8; 17];
            p[0] = 0xfd;
            p[1] = id;
            p[5..9].copy_from_slice(&20000u32.to_le_bytes());
            d.send(2, &p)?;
            d.send(2, &[0xfe, id])?;
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && d.features.iter().any(|v| v.is_none()) {
            if let Some((ch, p)) = d.packet()? {
                if ch == 2 {
                    d.control(&p)?;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if d.features[0] != Some(true) || d.features[1] != Some(true) {
            return Err(bad(
                "BNO085 did not enable orientation and calibrated gyro reports",
            ));
        }
        d.caps = Capabilities {
            orientation: true,
            gyro: true,
            accel: d.features[2] == Some(true),
        };
        Ok(d)
    }
    fn send(&mut self, ch: u8, p: &[u8]) -> io::Result<()> {
        let n = (p.len() + 4) as u16;
        let mut packet = vec![n as u8, (n >> 8) as u8, ch, self.tx[ch as usize]];
        packet.extend_from_slice(p);
        self.bus.write(&packet)?;
        self.tx[ch as usize] = self.tx[ch as usize].wrapping_add(1);
        Ok(())
    }
    fn packet(&mut self) -> io::Result<Option<(u8, Vec<u8>)>> {
        let h = match self.bus.read(4) {
            Ok(h) => h,
            // SH-2 may NACK a header read when there is no pending cargo.
            Err(e) if e.raw_os_error() == Some(libc::EREMOTEIO) => return Ok(None),
            Err(e) => return Err(e),
        };
        let raw = u16::from_le_bytes([h[0], h[1]]);
        let n = usize::from(raw & 0x7fff);
        if n == 0 {
            return Ok(None);
        }
        if !(4..=4096).contains(&n) || h[2] >= 6 {
            return Err(bad("invalid SHTP header"));
        }
        let p = self.bus.read(n)?;
        let seq = complete_transfer(&h, &p)?;
        let ch = h[2] as usize;
        if self.rx[ch].is_some_and(|last| {
            let delta = seq.wrapping_sub(last);
            delta == 0 || delta > 127
        }) {
            return Ok(None);
        }
        self.rx[ch] = Some(seq);
        Ok(Some((h[2], p[4..].to_vec())))
    }
    fn control(&mut self, p: &[u8]) -> io::Result<()> {
        let mut i = 0;
        while i < p.len() {
            let n = match p[i] {
                0xfc => 17,
                0xf8 | 0xf1 => 16,
                _ => return Err(bad("unknown BNO control report")),
            };
            if i + n > p.len() {
                return Err(bad("truncated BNO control report"));
            }
            if p[i] == 0xfc {
                if let Some(k) = [8, 2, 1].iter().position(|id| *id == p[i + 1]) {
                    self.features[k] =
                        Some(u32::from_le_bytes(p[i + 5..i + 9].try_into().unwrap()) > 0);
                }
            }
            i += n;
        }
        Ok(())
    }
}
impl Driver for Bno085 {
    fn capabilities(&self) -> Capabilities {
        self.caps
    }
    fn read(&mut self) -> io::Result<Sample> {
        let mut out = Sample::default();
        for _ in 0..8 {
            let received = Instant::now();
            let Some((ch, p)) = self.packet()? else {
                return Ok(out);
            };
            match ch {
                3 | 4 => {
                    let s = self.reports.parse(&p, received)?;
                    if s.quat.is_some() {
                        out.quat = s.quat;
                    }
                    if s.gyro.is_some() {
                        out.gyro = s.gyro;
                    }
                    if s.accel.is_some() {
                        out.accel = s.accel;
                    }
                }
                2 => self.control(&p)?,
                1 => return Err(bad("BNO085 reset; restart reader after checking hardware")),
                _ => {}
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "SHTP backlog; draining stale reports",
        ))
    }
    fn close(&mut self) -> io::Result<()> {
        self.bus.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(v: &str) -> Vec<u8> {
        (0..v.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&v[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn captured_header_only_transfer_and_continuation() {
        let h = hex("44000207");
        let p = hex(
            "44800208f8010206dca49800c100000006000000f800000696a498006e00000002000000f800030736a398000601000003000000f8000306e3a298001e01000005000000",
        );
        assert_eq!(complete_transfer(&h, &p).unwrap(), 8);
        assert!(complete_transfer(&h, &p[..p.len() - 1]).is_err());
        for offset in [0, 2, 3] {
            let mut bad = p.clone();
            bad[offset] ^= 1;
            assert!(complete_transfer(&h, &bad).is_err());
        }
        let mut same = p.clone();
        same[..4].copy_from_slice(&h);
        assert_eq!(complete_transfer(&h, &same).unwrap(), 7);
        let mut wrap = p.clone();
        wrap[3] = 0;
        assert_eq!(complete_transfer(&hex("440002ff"), &wrap).unwrap(), 0);
        assert!(complete_transfer(&p[..4], &p).is_err());
    }
    #[test]
    fn real_report_layout_units_and_independent_timestamps() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("imu_contract_vectors.json")).unwrap();
        let now = Instant::now();
        let mut r = Reports::default();
        let q = r
            .parse(&hex(f["bno"]["quat_hex"].as_str().unwrap()), now)
            .unwrap();
        assert!(q.gyro.is_none());
        let q = q.quat.unwrap();
        assert!((q.value[0] - 0.5f64.sqrt()).abs() < 1e-8);
        assert_eq!(now.duration_since(q.at), Duration::from_millis(10));
        let g = r
            .parse(&hex(f["bno"]["gyro_hex"].as_str().unwrap()), now)
            .unwrap();
        assert!(g.quat.is_none());
        let g = g.gyro.unwrap();
        assert_eq!(g.value, [1., -0.5, 0.25]);
        assert_eq!(now.duration_since(g.at), Duration::from_millis(4));
        assert!(
            r.parse(&hex(f["bno"]["gyro_hex"].as_str().unwrap()), now)
                .unwrap()
                .gyro
                .is_none()
        );
        let a = r
            .parse(&hex(f["bno"]["accel_hex"].as_str().unwrap()), now)
            .unwrap()
            .accel
            .unwrap();
        assert_eq!(a.value, [0., 0., 2511. / 256.]);
    }
    #[test]
    fn malformed_payload_is_transactional() {
        let now = Instant::now();
        let mut r = Reports::default();
        let mut p = hex("080403000000000000000040");
        let valid = p.clone();
        p.push(0x99);
        assert!(r.parse(&p, now).is_err());
        assert!(r.parse(&valid, now).unwrap().quat.is_some());
        for n in 1..12 {
            assert!(Reports::default().parse(&valid[..n], now).is_err());
        }
        assert!(r.parse(&hex("080503000000000000000000"), now).is_err());
        assert!(
            r.parse(&hex("080600000000000000000040"), now)
                .unwrap()
                .quat
                .is_none()
        );
        assert!(r.parse(&hex("02010301000200000000"), now).is_err());
    }
}
