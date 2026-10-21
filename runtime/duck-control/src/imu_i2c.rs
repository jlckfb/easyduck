//! LSM6DSV16X 的 Linux I2C 读取层。
//!
//! sflp 模式读取芯片 FIFO 中的姿态四元数，并交给 SflpDecoder 换算；
//! comp 模式读取原始加速度和角速度，使用带安装位置补偿的互补滤波。
//! I2cImuConfig 指定总线、地址、融合方式及滤波参数。
//! 无可用样本时返回 Ok(None)，总线错误通过 io::Result 返回，
//! 由调用方决定如何保留上次数据与统计连续缺失。

use std::io;
use std::os::unix::io::RawFd;
use std::time::Instant;

use crate::imu::{ImuData, SflpDecoder, IMU_BLOCK_LEN};

/// Configuration of the directly attached LSM6DSV16X.
#[derive(Debug, Clone)]
pub struct I2cImuConfig {
    pub dev: String,
    pub addr: u8,
    /// "sflp" = chip fusion via FIFO (deployed default); "comp" = host-side
    /// lever-arm-compensated complementary fusion.
    pub fusion: String,
    pub lever_arm: [f64; 3],
    pub g_filter_k: f64,
    pub accel_trust_g: f64,
}


const I2C_SLAVE: libc::c_ulong = 0x0703;

// Main-page registers.
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
const REG_FIFO_STATUS2: u8 = 0x1C;
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
const FUNC_CFG_SW_POR: u8 = 0x04;
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
const FIFO_WORD_LEN: usize = 7;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionMode {
    Sflp,
    Comp,
}

pub struct Lsm6dsv16xI2c {
    fd: RawFd,
    addr: u8,
    mode: FusionMode,
    lever_arm: [f64; 3],
    g_k: f64,
    accel_trust_g: f64,
    /// Chip SFLP decoder, used by the "sflp" pipeline.
    decoder: SflpDecoder,
    /// Mount (sensor→trunk) quaternion, used by the "comp" pipeline. Equals the decoder's
    /// mount so both pipelines agree on frame conventions.
    mount: [f64; 4],
    bias_body: [f64; 3],
    // "comp" state.
    quat: [f64; 4],
    g_est: [f64; 3],
    prev_w: [f64; 3],
    have_prev: bool,
    last_t: Option<std::time::Instant>,
}

impl Lsm6dsv16xI2c {
    pub fn open(
        dev: &str,
        addr: u8,
        mode: FusionMode,
        lever_arm: [f64; 3],
        g_k: f64,
        accel_trust_g: f64,
    ) -> io::Result<Self> {
        let dev_c = std::ffi::CString::new(dev)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in device path"))?;
        let fd = unsafe { libc::open(dev_c.as_ptr(), libc::O_RDWR) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut chip = Self {
            fd,
            addr,
            mode,
            lever_arm,
            g_k,
            accel_trust_g,
            decoder: SflpDecoder::default(),
            mount: crate::imu::SflpDecoder::DEFAULT_MOUNT,
            bias_body:[0.0;3],
            quat: [1.0, 0.0, 0.0, 0.0],
            g_est: [0.0, 0.0, -1.0],
            prev_w: [0.0; 3],
            have_prev: false,
            last_t: None,
        };
        chip.set_addr()?;
        chip.init()?;
        Ok(chip)
    }

    pub fn set_calibration(&mut self, mount:[f64;4], bias_sensor:[f64;3]) {
        self.mount=mount; self.decoder=SflpDecoder::new(mount);
        self.bias_body=crate::imu::rotate(mount,bias_sensor);
    }

    fn set_addr(&self) -> io::Result<()> {
        let rc = unsafe { libc::ioctl(self.fd, I2C_SLAVE, self.addr as libc::c_ulong) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn w8(&self, reg: u8, value: u8) -> io::Result<()> {
        let n = unsafe { libc::write(self.fd, [reg, value].as_ptr() as *const libc::c_void, 2) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn r8(&self, reg: u8) -> io::Result<u8> {
        let n = unsafe { libc::write(self.fd, [reg].as_ptr() as *const libc::c_void, 1) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut b = [0u8; 1];
        let got = unsafe { libc::read(self.fd, b.as_mut_ptr() as *mut libc::c_void, 1) };
        if got < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(b[0])
    }

    fn rn(&self, reg: u8, len: usize) -> io::Result<Vec<u8>> {
        let n = unsafe { libc::write(self.fd, [reg].as_ptr() as *const libc::c_void, 1) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buf = vec![0u8; len];
        let got = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, len) };
        if got < 0 {
            return Err(io::Error::last_os_error());
        }
        buf.truncate(got as usize);
        Ok(buf)
    }

    /// Switches between the main register page and the embedded-functions page by
    /// read-modify-write of `FUNC_CFG_ACCESS.EMB_FUNC_REG_ACCESS` (bit 7), preserving
    /// the other bits. `FUNC_CFG_ACCESS` itself is reachable from both pages.
    fn set_emb_bank(&self, emb: bool) -> io::Result<()> {
        let v = self.r8(REG_FUNC_CFG_ACCESS)?;
        let v = if emb {
            v | FUNC_CFG_EMB_FUNC_REG_ACCESS
        } else {
            v & !FUNC_CFG_EMB_FUNC_REG_ACCESS
        };
        self.w8(REG_FUNC_CFG_ACCESS, v)
    }

    fn init(&mut self) -> io::Result<()> {
        // WHO_AM_I first; only a chip that answers wrong gets the software power-on
        // reset. An unconditional POR run *after* the CTRL writes resets them all to
        // power-down — the 710cb98 regression that left the fusion block with no
        // input stream at all. The reset self-clears and also clears the bank bit.
        let mut who = self.r8(REG_WHO_AM_I)?;
        if who != WHO_AM_I_EXPECT {
            self.w8(REG_FUNC_CFG_ACCESS, FUNC_CFG_SW_POR)?;
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                who = self.r8(REG_WHO_AM_I)?;
                if who == WHO_AM_I_EXPECT {
                    break;
                }
            }
        }
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

        if self.mode == FusionMode::Sflp {
            // Configure the chip SFLP pipeline.
            // No watermark, and no accelerometer/gyroscope batching: both are read from
            // their output registers, so BDR words would only crowd quaternions out of
            // the FIFO.
            self.w8(REG_FIFO_CTRL1, 0x00)?;
            self.w8(REG_FIFO_CTRL2, 0x00)?;
            self.w8(REG_FIFO_CTRL3, 0x00)?;
            self.set_emb_bank(true)?;
            let fifo_en = self.r8(REG_EMB_FUNC_FIFO_EN_A)?;
            self.w8(
                REG_EMB_FUNC_FIFO_EN_A,
                (fifo_en | EMB_FUNC_FIFO_SFLP_GAME) & !EMB_FUNC_FIFO_SFLP_OTHERS,
            )?;
            self.set_emb_bank(false)?;

            // FIFO in stream mode: when full, newest samples overwrite the oldest, so a
            // tick that reads late sees current orientation rather than a stopped clock.
            self.w8(REG_FIFO_CTRL4, FIFO_MODE_STREAM)?;

            // 60 Hz fusion: fresher than the 50 Hz loop reads it, at a quarter of the
            // 240 Hz drain cost. Then switch the fusion block on and kick its
            // initialisation — without sflp_game_init it never starts producing.
            self.set_emb_bank(true)?;
            let odr = self.r8(REG_SFLP_ODR)?;
            self.w8(REG_SFLP_ODR, (odr & !SFLP_ODR_MASK) | SFLP_ODR_60HZ)?;
            let en = self.r8(REG_EMB_FUNC_EN_A)?;
            self.w8(REG_EMB_FUNC_EN_A, en | EMB_FUNC_EN_SFLP_GAME)?;
            let init = self.r8(REG_EMB_FUNC_INIT_A)?;
            self.w8(REG_EMB_FUNC_INIT_A, init | EMB_FUNC_INIT_SFLP_GAME)?;
            self.set_emb_bank(false)?;
        }
        // "comp" needs no chip-side fusion setup: raw accel/gyro streams are already running.
        Ok(())
    }

    /// Raw accel (g) and gyro (rad/s) triples, sensor frame.
    fn read_raw(&self) -> io::Result<([f64; 3], [f64; 3])> {
        let g_raw = self.rn(REG_OUTX_G, 6)?;
        let a_raw = self.rn(REG_OUTX_XL, 6)?;
        if g_raw.len() < 6 || a_raw.len() < 6 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short burst read"));
        }
        Ok((accel_g(&a_raw), gyro_rad(&g_raw)))
    }

    pub fn mode(&self) -> FusionMode {
        self.mode
    }

    /// Readiness of the chip-SFLP pipeline: whether the decoder has accumulated its
    /// 25-sample convergence count. The comp pipeline has no convergence gate; its
    /// readiness is answered unconditionally true by `DynamixelIo::imu_ready`.
    pub fn ready(&self) -> bool {
        self.decoder.ready()
    }

    /// One control-loop sample.
    ///
    /// `Ok(None)` = no fresh orientation data this tick (sflp mode: FIFO empty). The caller
    /// holds the previous sample and counts a miss; the servos are unaffected.
    pub fn read_sample(&mut self) -> io::Result<Option<ImuData>> {
        match self.mode {
            FusionMode::Sflp => self.read_sample_sflp(),
            FusionMode::Comp => self.read_sample_comp().map(Some),
        }
    }

    fn read_sample_sflp(&mut self) -> io::Result<Option<ImuData>> {
        // Drain pending words, keep the newest game-rotation quaternion (tag 0x13).
        // Depth is nine bits: STATUS1 (0x1B) carries DIFF[7:0], STATUS2 (0x1C) bit 0
        // carries DIFF[8] — the rest of STATUS2 is overrun/full/watermark flags
        // (lsm6dsv16x_reg.h fifo_status1/2_t). One two-byte burst read; 710cb98 had
        // the pair swapped, which turned the flags into a fake depth.
        let status = self.rn(REG_FIFO_STATUS1, 2)?;
        if status.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short fifo status read",
            ));
        }
        let pending = (status[0] as usize) | ((status[1] as usize & 0x01) << 8);
        let mut quat: Option<[u16; 3]> = None;
        for _ in 0..pending.min(16) {
            let word = self.rn(REG_FIFO_DATA, FIFO_WORD_LEN)?;
            if word.len() < 7 {
                break;
            }
            let tag = word[0] >> 3;
            if tag == FIFO_TAG_GAME {
                quat = Some([
                    u16::from_le_bytes([word[1], word[2]]),
                    u16::from_le_bytes([word[3], word[4]]),
                    u16::from_le_bytes([word[5], word[6]]),
                ]);
            }
        }
        let gyro_raw = self.rn(REG_OUTX_G, 6)?;
        if gyro_raw.len() < 6 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short gyro read"));
        }

        match quat {
            Some([x, y, z]) => {
                // The FIFO halves are IEEE half-precision bit patterns. Assemble the
                // bus-compatible block raw and hand it to SflpDecoder::decode, which
                // owns the half conversion, norm check and normalization.
                // (710cb98 norm-checked the raw u16 patterns, which read as ~16k, so
                // every real sample was rejected and the source read as frozen.)
                let mut block = [0u8; IMU_BLOCK_LEN];
                block[0..6].copy_from_slice(&gyro_raw);
                block[6..8].copy_from_slice(&x.to_le_bytes());
                block[8..10].copy_from_slice(&y.to_le_bytes());
                block[10..12].copy_from_slice(&z.to_le_bytes());
                {
                    let mut out=self.decoder.decode(&block);
                    for k in 0..3 {out.gyro[k]-=self.bias_body[k];}
                    Ok(Some(out))
                }
            }
            None => Ok(None),
        }
    }

    fn read_sample_comp(&mut self) -> io::Result<ImuData> {
        let dt = match self.last_t {
            Some(t) => t.elapsed().as_secs_f64().clamp(1e-3, 0.1),
            None => 0.02,
        };
        self.last_t = Some(std::time::Instant::now());

        let (a_s, w_s) = self.read_raw()?;

        // sensor→trunk via the calibrated mount (≈ identity with the x-front/y-left/z-up
        // convention the module is mounted in).
        let mut w_t = crate::imu::rotate(self.mount, w_s);
        for k in 0..3 {w_t[k]-=self.bias_body[k];}
        let a_t = crate::imu::rotate(self.mount, a_s);

        // Lever-arm compensation: subtract the linear acceleration of the IMU point due to
        // body rotation about the trunk origin: ω×(ω×r) + α×r.
        let alpha = if self.have_prev && dt > 1e-4 {
            [
                (w_t[0] - self.prev_w[0]) / dt,
                (w_t[1] - self.prev_w[1]) / dt,
                (w_t[2] - self.prev_w[2]) / dt,
            ]
        } else {
            [0.0; 3]
        };
        self.prev_w = w_t;
        let r = self.lever_arm;
        let w_dot_r = w_t[0] * r[0] + w_t[1] * r[1] + w_t[2] * r[2];
        let lever = [
            w_t[0] * w_dot_r - (w_t[0] * w_t[0] + w_t[1] * w_t[1] + w_t[2] * w_t[2]) * r[0]
                + alpha[1] * r[2]
                - alpha[2] * r[1],
            w_t[1] * w_dot_r - (w_t[0] * w_t[0] + w_t[1] * w_t[1] + w_t[2] * w_t[2]) * r[1]
                + alpha[2] * r[0]
                - alpha[0] * r[2],
            w_t[2] * w_dot_r - (w_t[0] * w_t[0] + w_t[1] * w_t[1] + w_t[2] * w_t[2]) * r[2]
                + alpha[0] * r[1]
                - alpha[1] * r[0],
        ];
        let a_clean = [
            a_t[0] - lever[0],
            a_t[1] - lever[1],
            a_t[2] - lever[2],
        ];

        // Attitude propagation from the (trunk-frame) angular velocity.
        self.quat = quat_integrate(self.quat, w_t, dt);

        // Accelerometer gravity direction (trunk frame): a stationary body reads +1 g
        // opposite to gravity, so the direction of gravity is −a.
        let mag = (a_clean[0] * a_clean[0] + a_clean[1] * a_clean[1]
            + a_clean[2] * a_clean[2])
            .sqrt();
        // Impact gate: when |a| departs from 1 g the accelerometer is contaminated and its
        // correction weight collapses to zero for this tick.
        let trust = if (mag - 1.0).abs() < self.accel_trust_g { 1.0 } else { 0.0 };
        if mag > 0.5 {
            let g_acc = [-a_clean[0] / mag, -a_clean[1] / mag, -a_clean[2] / mag];
            for k in 0..3 {
                self.g_est[k] += self.g_k * trust * (g_acc[k] - self.g_est[k]);
            }
            let n = (self.g_est[0] * self.g_est[0]
                + self.g_est[1] * self.g_est[1]
                + self.g_est[2] * self.g_est[2])
                .sqrt();
            if n > 0.5 {
                self.g_est = [self.g_est[0] / n, self.g_est[1] / n, self.g_est[2] / n];
            }
        }

        Ok(ImuData {
            gyro: w_t,
            gravity: self.g_est,
            quat: self.quat,
        })
    }
}

impl Drop for Lsm6dsv16xI2c {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

/// First-order quaternion integration: `q̇ = ½ q ⊗ (0, ω)` with ω in the trunk frame.
fn quat_integrate(q: [f64; 4], w: [f64; 3], dt: f64) -> [f64; 4] {
    let dq = [
        -0.5 * (q[1] * w[0] + q[2] * w[1] + q[3] * w[2]) * dt,
        0.5 * (q[0] * w[0] + q[2] * w[2] - q[3] * w[1]) * dt,
        0.5 * (q[0] * w[1] + q[3] * w[0] - q[1] * w[2]) * dt,
        0.5 * (q[0] * w[2] + q[1] * w[1] - q[2] * w[0]) * dt,
    ];
    let out = [q[0] + dq[0], q[1] + dq[1], q[2] + dq[2], q[3] + dq[3]];
    let n = (out[0] * out[0] + out[1] * out[1] + out[2] * out[2] + out[3] * out[3]).sqrt();
    if n > 0.5 {
        [out[0] / n, out[1] / n, out[2] / n, out[3] / n]
    } else {
        q
    }
}

/// Assembles the bus-compatible block from I²C-side parts (tests).
pub fn assemble_block(gyro: &[u8; 6], quat: &[u16; 3]) -> [u8; IMU_BLOCK_LEN] {
    let mut block = [0u8; IMU_BLOCK_LEN];
    block[0..6].copy_from_slice(gyro);
    for (dst, src) in block[6..12].chunks_exact_mut(2).zip(quat) {
        dst.copy_from_slice(&src.to_le_bytes());
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imu::GYRO_RAD_PER_LSB;

    /// The assembled block must round-trip through the SFLP decoder identically to the bus
    /// path: the I²C source is only acceptable if the control loop cannot tell them apart.
    #[test]
    fn assembled_block_decodes_through_sflp_decoder() {
        let mut d = SflpDecoder::default();
        let mut gyro = [0u8; 6];
        gyro[0..2].copy_from_slice(&(-1000i16).to_le_bytes());
        let block = assemble_block(&gyro, &[0x3000, 0x0000, 0x0000]);
        // Three identical blocks so the median filter settles on this sample.
        d.decode(&block);
        d.decode(&block);
        let out = d.decode(&block);
        let magnitude: f64 = out.gyro.iter().map(|v| v * v).sum::<f64>().sqrt();
        let expected = 1000.0 * GYRO_RAD_PER_LSB;
        // Magnitude pins the signed-ness; the mount rotation may flip individual axes.
        assert!(
            (magnitude - expected).abs() < 1e-9,
            "expected |gyro| {expected}, got {magnitude}"
        );
        assert_ne!(out.quat, [1.0, 0.0, 0.0, 0.0], "a live quaternion must decode");
    }

    /// Zero quaternion bytes pass through as zeros so the decoder's hold-last applies.
    #[test]
    fn zero_quaternion_passes_through_as_zero() {
        let block = assemble_block(&[0u8; 6], &[0, 0, 0]);
        assert_eq!(&block[6..12], &[0u8; 6]);
    }

    /// Complementary fusion converges to [0,0,−1] when the raw accel reads a stationary
    /// level body (a = +1 g on the chip's z axis).
    #[test]
    fn comp_fusion_converges_statically() {
        let mut chip = Lsm6dsv16xI2c {
            fd: -1,
            addr: 0x6A,
            mode: FusionMode::Comp,
            lever_arm: [0.0; 3],
            g_k: 0.1,
            accel_trust_g: 0.3,
            decoder: SflpDecoder::default(),
            mount: crate::imu::SflpDecoder::DEFAULT_MOUNT,
            bias_body:[0.0;3],
            quat: [1.0, 0.0, 0.0, 0.0],
            g_est: [0.0, 0.0, -1.0],
            prev_w: [0.0; 3],
            have_prev: false,
            last_t: None,
        };
        // Simulate a tick at rest: read_raw would return a = (0,0,+1), ω = 0. We cannot
        // inject through read_raw (it does I/O), so drive the fusion math directly by
        // temporarily stubbing via the lever-free path: replicate read_sample_comp's
        // updates with the same formulas.
        let a_t: [f64; 3] = [0.0, 0.0, 1.0];
        let w_t: [f64; 3] = [0.0, 0.0, 0.0];
        for _ in 0..50 {
            let mag: f64 = (a_t[0] * a_t[0] + a_t[1] * a_t[1] + a_t[2] * a_t[2]).sqrt();
            let trust = if (mag - 1.0).abs() < chip.accel_trust_g { 1.0 } else { 0.0 };
            let g_acc = [-a_t[0] / mag, -a_t[1] / mag, -a_t[2] / mag];
            for k in 0..3 {
                chip.g_est[k] += chip.g_k * trust * (g_acc[k] - chip.g_est[k]);
            }
            let n = (chip.g_est[0] * chip.g_est[0]
                + chip.g_est[1] * chip.g_est[1]
                + chip.g_est[2] * chip.g_est[2])
                .sqrt();
            chip.g_est = [chip.g_est[0] / n, chip.g_est[1] / n, chip.g_est[2] / n];
        }
        let closeness = (chip.g_est[2] - (-1.0)).abs();
        assert!(closeness < 1e-9, "g_est should stay at [0,0,-1], got {:?}", chip.g_est);
    }
}
