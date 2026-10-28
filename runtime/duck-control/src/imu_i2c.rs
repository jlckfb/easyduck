//! LSM6DSV16X register driver. Raw sensor frame only; processing lives in imu_driver.
pub use crate::imu_driver::I2cImuConfig;
use crate::imu_driver::{Capabilities, Driver, Sample, Timed};
use crate::imu_transport::I2c;
use std::{io, time::Instant};
const REG_FUNC_CFG_ACCESS: u8 = 0x01;
const REG_WHO_AM_I: u8 = 0x0F;
const REG_CTRL1_XL: u8 = 0x10;
const REG_CTRL2_G: u8 = 0x11;
const REG_CTRL3_C: u8 = 0x12;
const REG_CTRL6_C: u8 = 0x15;
const REG_CTRL8_XL: u8 = 0x17;
const REG_OUTX_G: u8 = 0x22;
const REG_OUTX_XL: u8 = 0x28;
const REG_FIFO_STATUS1: u8 = 0x1B;
const REG_FIFO_CTRL1: u8 = 0x07;
const REG_FIFO_CTRL2: u8 = 0x08;
const REG_FIFO_CTRL3: u8 = 0x09;
const REG_FIFO_CTRL4: u8 = 0x0A;
const REG_FIFO_DATA: u8 = 0x78;

// Embedded-page registers (reached via FUNC_CFG_ACCESS bit7).
const REG_EMB_FUNC_EN_A: u8 = 0x04;
const REG_EMB_FUNC_FIFO_EN_A: u8 = 0x44;
const REG_SFLP_ODR: u8 = 0x5E;
const REG_EMB_FUNC_INIT_A: u8 = 0x66;

const FUNC_CFG_EMB_FUNC_REG_ACCESS: u8 = 0x80;
const EMB_FUNC_EN_SFLP_GAME: u8 = 0x02;
const EMB_FUNC_INIT_SFLP_GAME: u8 = 0x02;
const EMB_FUNC_FIFO_SFLP_GAME: u8 = 0x02;
const EMB_FUNC_FIFO_SFLP_OTHERS: u8 = 0x30;
const SFLP_ODR_MASK: u8 = 0x38;
const SFLP_ODR_60HZ: u8 = 0x2 << 3;
const CTRL3_BDU_IF_INC: u8 = 0x44;
const XL_GY_ODR_240HZ: u8 = 0x07;
const CTRL6_C_FS_500DPS: u8 = 0x02;
const CTRL8_XL_FS_4G: u8 = 0x01;
const FIFO_MODE_STREAM: u8 = 0x06;
const FIFO_TAG_GAME: u8 = 0x13; // bits[7:3] of the tag byte
const WHO_AM_I_EXPECT: u8 = 0x70;

/// ±500 dps gyro, i16 LE raw bytes → rad/s triple.
fn gyro_rad(raw: &[u8]) -> [f64; 3] {
    [
        i16::from_le_bytes([raw[0], raw[1]]) as f64 * GYRO_RAD_PER_LSB,
        i16::from_le_bytes([raw[2], raw[3]]) as f64 * GYRO_RAD_PER_LSB,
        i16::from_le_bytes([raw[4], raw[5]]) as f64 * GYRO_RAD_PER_LSB,
    ]
}

/// ±4 g accelerometer, i16 LE raw bytes → g triple.
fn accel_g(raw: &[u8]) -> [f64; 3] {
    [
        i16::from_le_bytes([raw[0], raw[1]]) as f64 * ACCEL_G_PER_LSB,
        i16::from_le_bytes([raw[2], raw[3]]) as f64 * ACCEL_G_PER_LSB,
        i16::from_le_bytes([raw[4], raw[5]]) as f64 * ACCEL_G_PER_LSB,
    ]
}

// Scale factors. Gyro ±500 dps (matches the bus block), accel ±4 g.
const GYRO_RAD_PER_LSB: f64 = 0.0175 * std::f64::consts::PI / 180.0;
const ACCEL_G_PER_LSB: f64 = 0.122 / 1000.0;

pub struct Lsm6dsv16xI2c {
    bus: I2c,
    sflp: bool,
    settled: u32,
}
impl Lsm6dsv16xI2c {
    pub fn open(c: &I2cImuConfig) -> io::Result<Self> {
        let mut d = Self {
            bus: I2c::open(&c.dev, c.addr)?,
            sflp: c.fusion == "sflp",
            settled: 0,
        };
        d.init()?;
        Ok(d)
    }
    fn w8(&self, r: u8, v: u8) -> io::Result<()> {
        self.bus.write(&[r, v])
    }
    fn r8(&self, r: u8) -> io::Result<u8> {
        Ok(self.rn(r, 1)?[0])
    }
    fn rn(&self, r: u8, n: usize) -> io::Result<Vec<u8>> {
        self.bus.write(&[r])?;
        self.bus.read(n)
    }
    fn set_emb_bank(&self, emb: bool) -> io::Result<()> {
        let v = self.r8(REG_FUNC_CFG_ACCESS)?;
        let v = if emb {
            v | FUNC_CFG_EMB_FUNC_REG_ACCESS
        } else {
            v & !FUNC_CFG_EMB_FUNC_REG_ACCESS
        };
        self.w8(REG_FUNC_CFG_ACCESS, v)
    }

    fn embedded(&self, operation: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        self.set_emb_bank(true)?;
        let result = operation();
        let restored = self.set_emb_bank(false);
        result.and(restored)
    }

    fn init(&mut self) -> io::Result<()> {
        // Verify identity before configuring registers; never reset an unexpected device.
        let who = self.r8(REG_WHO_AM_I)?;
        if who != WHO_AM_I_EXPECT {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("WHO_AM_I read {who:#04x}, expected {WHO_AM_I_EXPECT:#04x}"),
            ));
        }

        // Block data update (read-side consistency) and auto-increment (multi-byte
        // reads).
        self.w8(REG_CTRL3_C, CTRL3_BDU_IF_INC)?;
        // Gyro ±500 dps — the scaling `crate::imu` decodes against — and accel ±4 g.
        self.w8(REG_CTRL6_C, CTRL6_C_FS_500DPS)?;
        self.w8(REG_CTRL8_XL, CTRL8_XL_FS_4G)?;
        // XL and GY at 240 Hz: the SFLP consumes both, and the gyroscope bytes are read
        // fresh from their output registers every tick. Neither is batched, so the rate
        // costs the drain nothing.
        self.w8(REG_CTRL1_XL, XL_GY_ODR_240HZ)?;
        self.w8(REG_CTRL2_G, XL_GY_ODR_240HZ)?;

        if self.sflp {
            // Configure the chip SFLP pipeline.
            // No watermark, and no accelerometer/gyroscope batching: both are read from
            // their output registers, so BDR words would only crowd quaternions out of
            // the FIFO.
            self.w8(REG_FIFO_CTRL1, 0x00)?;
            self.w8(REG_FIFO_CTRL2, 0x00)?;
            self.w8(REG_FIFO_CTRL3, 0x00)?;
            self.embedded(|| {
                let fifo_en = self.r8(REG_EMB_FUNC_FIFO_EN_A)?;
                self.w8(
                    REG_EMB_FUNC_FIFO_EN_A,
                    (fifo_en | EMB_FUNC_FIFO_SFLP_GAME) & !EMB_FUNC_FIFO_SFLP_OTHERS,
                )?;
                Ok(())
            })?;

            // FIFO in stream mode: when full, newest samples overwrite the oldest, so a
            // tick that reads late sees current orientation rather than a stopped clock.
            self.w8(REG_FIFO_CTRL4, FIFO_MODE_STREAM)?;

            // 60 Hz fusion: fresher than the 50 Hz loop reads it, at a quarter of the
            // 240 Hz drain cost. Then switch the fusion block on and kick its
            // initialisation — without sflp_game_init it never starts producing.
            self.embedded(|| {
                let odr = self.r8(REG_SFLP_ODR)?;
                self.w8(REG_SFLP_ODR, (odr & !SFLP_ODR_MASK) | SFLP_ODR_60HZ)?;
                let en = self.r8(REG_EMB_FUNC_EN_A)?;
                self.w8(REG_EMB_FUNC_EN_A, en | EMB_FUNC_EN_SFLP_GAME)?;
                let init = self.r8(REG_EMB_FUNC_INIT_A)?;
                self.w8(REG_EMB_FUNC_INIT_A, init | EMB_FUNC_INIT_SFLP_GAME)?;
                Ok(())
            })?;
        }
        // "comp" needs no chip-side fusion setup: raw accel/gyro streams are already running.
        Ok(())
    }
}
fn half(h: u16) -> f64 {
    let sign = if h & 0x8000 == 0 { 1. } else { -1. };
    let exp = (h >> 10) & 31;
    let mant = f64::from(h & 1023);
    sign * if exp == 0 {
        mant * 2f64.powi(-24)
    } else if exp == 31 {
        f64::NAN
    } else {
        (1. + mant / 1024.) * 2f64.powi(i32::from(exp) - 15)
    }
}
pub fn decode_sflp(word: &[u8]) -> io::Result<[f64; 4]> {
    if word.len() != 7 || word[0] >> 3 != FIFO_TAG_GAME {
        return Err(io::Error::other("invalid SFLP word"));
    }
    let v: [f64; 3] =
        std::array::from_fn(|i| half(u16::from_le_bytes([word[1 + 2 * i], word[2 + 2 * i]])));
    let n = v.iter().map(|x| x * x).sum::<f64>();
    if !n.is_finite() || n > 1.002 {
        return Err(io::Error::other("invalid SFLP quaternion"));
    }
    crate::imu_driver::normalized([(1. - n).max(0.).sqrt(), v[0], v[1], v[2]])
}
impl Driver for Lsm6dsv16xI2c {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            orientation: self.sflp,
            gyro: true,
            accel: true,
        }
    }
    fn read(&mut self) -> io::Result<Sample> {
        let mut s = Sample::default();
        let flags = self.r8(0x1e)?;
        if flags & 2 != 0 {
            s.gyro = Some(Timed {
                value: gyro_rad(&self.rn(REG_OUTX_G, 6)?),
                at: Instant::now(),
            });
        }
        if flags & 1 != 0 {
            s.accel = Some(Timed {
                value: accel_g(&self.rn(REG_OUTX_XL, 6)?).map(|x| x * 9.80665),
                at: Instant::now(),
            });
        }
        if self.sflp {
            let status = self.rn(REG_FIFO_STATUS1, 2)?;
            let pending = usize::from(status[0]) | (usize::from(status[1] & 1) << 8);
            // A bounded drain discards backlog rather than labelling old FIFO words fresh.
            let overloaded = pending > 16 || status[1] & 0x40 != 0;
            for _ in 0..pending.min(16) {
                let w = self.rn(REG_FIFO_DATA, 7)?;
                if !overloaded && w[0] >> 3 == FIFO_TAG_GAME {
                    let value = decode_sflp(&w)?;
                    self.settled = self.settled.saturating_add(1);
                    // Preserve the LSM startup settling gate using actual FIFO reports.
                    if self.settled >= 25 {
                        s.quat = Some(Timed {
                            value,
                            at: Instant::now(),
                        });
                    }
                }
            }
            if overloaded {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "LSM FIFO backlog; draining stale samples",
                ));
            }
        }
        Ok(s)
    }
    fn close(&mut self) -> io::Result<()> {
        self.bus.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sflp_packet_norm_and_invalid_half() {
        assert_eq!(
            decode_sflp(&[0x98, 0, 0, 0, 0, 0, 0]).unwrap(),
            [1., 0., 0., 0.]
        );
        assert!(decode_sflp(&[0x98, 0, 0x7c, 0, 0, 0, 0]).is_err());
        assert!(decode_sflp(&[0x98, 0, 0x3c, 0, 0x3c, 0, 0]).is_err());
        assert!(decode_sflp(&[0; 6]).is_err());
    }
    #[test]
    fn lsm_si_scales() {
        assert!(
            (gyro_rad(&[1, 0, 0, 0, 0, 0])[0] - 0.0175 * std::f64::consts::PI / 180.).abs() < 1e-12
        );
        assert!((accel_g(&[1, 0, 0, 0, 0, 0])[0] * 9.80665 - 0.000122 * 9.80665).abs() < 1e-12);
    }
}
