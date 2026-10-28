//! IMU 数据与姿态换算。
//!
//! ImuData 输出躯干坐标系的角速度（rad/s）、单位重力向量及
//! 标量在前的姿态四元数 [w, x, y, z]。直立时重力为 [0, 0, -1]。
//! SflpDecoder 解码 I2C 读取层组装的 12 字节数据：
//! 前 6 字节为陀螺仪 i16 小端值，后 6 字节为 SFLP 四元数的
//! x/y/z 半精度分量。安装旋转、归一化、异常样本处理在此完成。

use crate::model::NUM_JOINTS;

/// Bytes of the v2 board's block consumed per tick.
pub const IMU_BLOCK_LEN: usize = 12;

/// What the control loop knows about the robot's orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImuData {
    /// Angular velocity in the trunk frame, rad/s.
    pub gyro: [f64; 3],
    /// Projected gravity in the trunk frame, unit vector. Upright is `[0, 0, -1]`.
    ///
    /// This is what the policy observes, and what fall detection thresholds on.
    pub gravity: [f64; 3],
    /// Orientation, trunk→world, scalar-first `[w, x, y, z]`.
    pub quat: [f64; 4],
}

impl Default for ImuData {
    fn default() -> Self {
        Self {
            gyro: [0.0; 3],
            gravity: [0.0, 0.0, -1.0],
            quat: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

/// ±500 dps at 17.5 mdps/LSB, in rad/s.
pub const GYRO_RAD_PER_LSB: f64 = 0.0175 * std::f64::consts::PI / 180.0;

/// Decodes the board's block into [`ImuData`].
///
/// Stateful only for spike rejection and for holding the last good quaternion — there is
/// no filter here. Keeping the state explicit means [`crate::io::FakeIo`] can drive the
/// same decoder in tests.
pub struct SflpDecoder {
    /// Sensor→trunk mounting rotation, scalar-first.
    mount: [f64; 4],
    /// Last quaternion the board actually produced. Held across blocks that arrive before
    /// SFLP has written its table — snapping to identity would report a robot as upright
    /// when its orientation is simply unknown, which is the worst possible lie to tell
    /// fall detection.
    last_quat: [f64; 4],
    /// Blocks carrying a live quaternion. Gates [`SflpDecoder::ready`].
    quat_samples: u32,
    /// The most recent decode output. The I²C source holds this on ticks its FIFO carries no
    /// fresh quaternion — the same hold-the-last-sample behaviour the bus path gets from the
    /// chip itself repeating a block.
    last: ImuData,
    gyro_history: [[f64; 3]; 2],
    gravity_history: [[f64; 3]; 2],
}

impl Default for SflpDecoder {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MOUNT)
    }
}

impl SflpDecoder {
    /// 传感器到躯干坐标系的后备安装旋转，格式为 [w, x, y, z]。
    /// 此常量适用于既定安装方式；其他机器应提供自己的 IMU 标定。
    /// 运行时传入的安装旋转优先于该后备值。
    pub const DEFAULT_MOUNT: [f64; 4] = [
        0.999901,
        0.003545,
        -0.005088,
        -0.012669,
    ];

    pub fn new(mount: [f64; 4]) -> Self {
        // Normalise the mount on the way in. Every use below — `rotate`, `rotate_inverse` —
        // is only a rotation for a unit quaternion, and a mount that is short by even 2e-7 (as
        // a hand-computed one is) silently scales the vectors it touches. That is exactly what
        // `gyro_counts_are_signed` caught when the mount picked up a levelling tilt: the
        // magnitude drifted by 1.4e-7 and the test failed on a 1e-9 tolerance.
        let norm = (mount[0] * mount[0] + mount[1] * mount[1]
            + mount[2] * mount[2] + mount[3] * mount[3])
            .sqrt();
        let mount = if norm > 0.0 {
            [mount[0] / norm, mount[1] / norm, mount[2] / norm, mount[3] / norm]
        } else {
            [1.0, 0.0, 0.0, 0.0]
        };
        Self {
            mount,
            last_quat: [1.0, 0.0, 0.0, 0.0],
            quat_samples: 0,
            last: ImuData::default(),
            gyro_history: [[0.0; 3]; 2],
            gravity_history: [[0.0, 0.0, -1.0]; 2],
        }
    }

    /// Whether the chip has demonstrably produced fused output — roughly 0.25 s at 100 Hz.
    ///
    /// Until this is true the orientation is a default, not a measurement. Slice 2's fall
    /// detection must not run before it.
    pub fn ready(&self) -> bool {
        self.quat_samples >= 25
    }

    /// The most recent decoded sample. Served on ticks whose source produced nothing new —
    /// the hold-the-last-sample behaviour the bus board exhibits by repeating a block.
    pub fn last(&self) -> ImuData {
        self.last
    }

    pub fn decode(&mut self, raw: &[u8; IMU_BLOCK_LEN]) -> ImuData {
        self.decode_inner(raw, false)
    }

    /// A tagged FIFO entry is a new sample, even for an exact identity quaternion.
    /// Malformed quaternions must not refresh the orientation age.
    pub fn decode_fifo(&mut self, raw: &[u8; IMU_BLOCK_LEN]) -> Option<ImuData> {
        let before = self.quat_samples;
        let out = self.decode_inner(raw, true);
        (self.quat_samples > before).then_some(out)
    }

    fn decode_inner(&mut self, raw: &[u8; IMU_BLOCK_LEN], fifo_entry: bool) -> ImuData {
        let gyro_sensor = [
            i16::from_le_bytes([raw[0], raw[1]]) as f64 * GYRO_RAD_PER_LSB,
            i16::from_le_bytes([raw[2], raw[3]]) as f64 * GYRO_RAD_PER_LSB,
            i16::from_le_bytes([raw[4], raw[5]]) as f64 * GYRO_RAD_PER_LSB,
        ];
        let gyro = rotate(self.mount, gyro_sensor);

        // All-zero quaternion bytes mean SFLP has not written its table yet — the board
        // just powered up, or its init failed. Keep the last good value.
        let packed = [
            u16::from_le_bytes([raw[6], raw[7]]),
            u16::from_le_bytes([raw[8], raw[9]]),
            u16::from_le_bytes([raw[10], raw[11]]),
        ];
        if fifo_entry || packed != [0, 0, 0] {
            let (x, y, z) = (half(packed[0]), half(packed[1]), half(packed[2]));
            let norm_sq = x * x + y * y + z * z;
            // A quaternion the chip never produced would fail this; ≤ 1.02 allows for
            // half-precision rounding at full scale.
            if x.is_finite() && y.is_finite() && z.is_finite() && norm_sq <= 1.02 {
                let w = (1.0 - norm_sq).max(0.0).sqrt();
                let mount_inv = [
                    self.mount[0],
                    -self.mount[1],
                    -self.mount[2],
                    -self.mount[3],
                ];
                let q = mul([w, x, y, z], mount_inv);
                let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                if norm > 0.5 {
                    self.last_quat = [q[0] / norm, q[1] / norm, q[2] / norm, q[3] / norm];
                    self.quat_samples = self.quat_samples.saturating_add(1);
                }
            }
        }

        // Normalise *before* the median, matching the runtime. Note the consequence: a
        // component-wise median across three unit vectors is not itself unit-norm, so
        // during a transient the policy sees a slightly short vector. Steady state is
        // exact. Whether the training env expects strict unit norm is worth settling when
        // slice 2 wires up observations — normalising after the median would guarantee it,
        // but that is a behaviour change to a path that currently walks.
        let gravity = normalise(rotate_inverse(self.last_quat, [0.0, 0.0, -1.0]));

        let out = ImuData {
            gyro: median3_each(&self.gyro_history, gyro),
            gravity: median3_each(&self.gravity_history, gravity),
            quat: self.last_quat,
        };
        self.gyro_history = [self.gyro_history[1], gyro];
        self.gravity_history = [self.gravity_history[1], gravity];
        self.last = out;
        out
    }
}

/// Single-sample spike rejection. A dropped or corrupted block shows up as one wild value;
/// a median over three discards it without the lag of an average.
fn median3_each(history: &[[f64; 3]; 2], now: [f64; 3]) -> [f64; 3] {
    let m = |a: f64, b: f64, c: f64| a.max(b).min(c).max(a.min(b));
    [
        m(history[0][0], history[1][0], now[0]),
        m(history[0][1], history[1][1], now[1]),
        m(history[0][2], history[1][2], now[2]),
    ]
}

/// IEEE 754 binary16 → f64. The LSM6DSV16X ships quaternion components in this format.
fn half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((bits >> 10) & 0x1F) as i32;
    let frac = (bits & 0x3FF) as f64;
    match exp {
        0 => sign * frac * 2.0_f64.powi(-24),
        0x1F if frac == 0.0 => sign * f64::INFINITY,
        0x1F => f64::NAN,
        _ => sign * (1.0 + frac / 1024.0) * 2.0_f64.powi(exp - 15),
    }
}

/// Hamilton product, scalar-first.
pub(crate) fn mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let ([aw, ax, ay, az], [bw, bx, by, bz]) = (a, b);
    [
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    ]
}

/// `q · v · q⁻¹`
pub fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let (t, c) = cross_terms(q, v);
    [
        v[0] + q[0] * t[0] + c[0],
        v[1] + q[0] * t[1] + c[1],
        v[2] + q[0] * t[2] + c[2],
    ]
}

/// `q⁻¹ · v · q` — world vector expressed in the body frame.
pub(crate) fn rotate_inverse(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let (t, c) = cross_terms(q, v);
    [
        v[0] - q[0] * t[0] + c[0],
        v[1] - q[0] * t[1] + c[1],
        v[2] - q[0] * t[2] + c[2],
    ]
}

fn cross_terms(q: [f64; 4], v: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (x, y, z) = (q[1], q[2], q[3]);
    let t = [
        (y * v[2] - z * v[1]) * 2.0,
        (z * v[0] - x * v[2]) * 2.0,
        (x * v[1] - y * v[0]) * 2.0,
    ];
    let c = [
        y * t[2] - z * t[1],
        z * t[0] - x * t[2],
        x * t[1] - y * t[0],
    ];
    (t, c)
}

/// Unit vector, falling back to "upright" rather than dividing by ~zero.
fn normalise(v: [f64; 3]) -> [f64; 3] {
    let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if mag > 0.1 {
        [v[0] / mag, v[1] / mag, v[2] / mag]
    } else {
        [0.0, 0.0, -1.0]
    }
}

/// Compile-time assurance that the joint count and this module stay independent — the IMU
/// block is fixed-size regardless of how many servos share the bus.
const _: () = assert!(NUM_JOINTS > 0);

#[cfg(test)]
mod tests {
    use super::*;

    /// Half-precision decoding is hand-rolled, so pin the cases that matter: zero, one,
    /// a negative, and a subnormal. A wrong exponent bias here silently tilts the horizon.
    #[test]
    fn half_precision_decodes_known_values() {
        assert_eq!(half(0x0000), 0.0);
        assert_eq!(half(0x3C00), 1.0);
        assert_eq!(half(0xBC00), -1.0);
        assert_eq!(half(0x3800), 0.5);
        assert!((half(0x3555) - 0.333).abs() < 1e-3);
    }

    /// A block of zeroes is the board saying "SFLP has not started". Reporting identity —
    /// i.e. perfectly upright — would tell fall detection the robot is fine when its
    /// orientation is simply unknown.
    #[test]
    fn all_zero_quaternion_bytes_hold_the_last_good_value() {
        let mut d = SflpDecoder::default();
        assert!(!d.ready());

        let zero = d.decode(&[0u8; IMU_BLOCK_LEN]);
        assert_eq!(zero.quat, [1.0, 0.0, 0.0, 0.0]);
        assert!(!d.ready(), "a zero block must not count as a live sample");

        // 0x3C00 = 1.0 in the x slot would exceed unit norm on its own; use a modest
        // rotation the chip could actually emit.
        let mut block = [0u8; IMU_BLOCK_LEN];
        block[6..8].copy_from_slice(&0x3000u16.to_le_bytes()); // x = 0.125
        let live = d.decode(&block);
        assert_ne!(live.quat, [1.0, 0.0, 0.0, 0.0]);

        let held = d.decode(&[0u8; IMU_BLOCK_LEN]);
        assert_eq!(held.quat, live.quat, "zero block must hold, not reset");
    }

    /// `ready()` gates fall detection in slice 2. If it were true from the first block,
    /// the robot would be judged on a default orientation for the first quarter second.
    #[test]
    fn not_ready_until_the_chip_has_produced_output() {
        let mut d = SflpDecoder::default();
        let mut block = [0u8; IMU_BLOCK_LEN];
        block[6..8].copy_from_slice(&0x3000u16.to_le_bytes());
        for _ in 0..24 {
            d.decode(&block);
        }
        assert!(!d.ready());
        d.decode(&block);
        assert!(d.ready());
    }

    /// In steady state gravity must be a unit vector at any orientation — the policy
    /// observes it directly and was trained on normalised input.
    ///
    /// Three identical blocks per orientation so the median settles. Mid-transient the
    /// median blends three different unit vectors component-wise and the result is
    /// slightly short; that is the runtime's behaviour and is deliberately preserved (see
    /// the note in `decode`).
    #[test]
    fn gravity_is_a_unit_vector_in_steady_state() {
        let mut d = SflpDecoder::default();
        let mut block = [0u8; IMU_BLOCK_LEN];
        for packed in [0x0000u16, 0x3000, 0x3800, 0xB000] {
            block[6..8].copy_from_slice(&packed.to_le_bytes());
            d.decode(&block);
            d.decode(&block);
            let g = d.decode(&block).gravity;
            let mag = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
            assert!(
                (mag - 1.0).abs() < 1e-9,
                "gravity magnitude {mag} for {packed:#06x}"
            );
        }
    }

    /// The transient case, pinned so the bound is known rather than assumed: switching
    /// orientation between ticks blends three vectors and shortens the result. If this
    /// ever gets far from 1.0 the policy is being fed something training never saw.
    #[test]
    fn gravity_stays_close_to_unit_through_a_transient() {
        let mut d = SflpDecoder::default();
        let mut block = [0u8; IMU_BLOCK_LEN];
        block[6..8].copy_from_slice(&0x0000u16.to_le_bytes());
        d.decode(&block);
        block[6..8].copy_from_slice(&0x3800u16.to_le_bytes());
        let g = d.decode(&block).gravity;
        let mag = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
        assert!(mag > 0.5, "transient gravity collapsed to {mag}");
        assert!(mag <= 1.0 + 1e-9);
    }

    /// Gyro counts are signed. Reading them as unsigned would turn every negative rate
    /// into a large positive one, which reads as a robot spinning.
    #[test]
    fn gyro_counts_are_signed() {
        let mut d = SflpDecoder::default();
        let mut block = [0u8; IMU_BLOCK_LEN];
        block[0..2].copy_from_slice(&(-1000i16).to_le_bytes());
        // Three identical blocks so the median settles on this sample.
        d.decode(&block);
        d.decode(&block);
        let out = d.decode(&block);
        // Magnitude is the L2 norm, not the sum of |components|: the gyro is rotated by
        // `mount` before it leaves the decoder, and a rotation preserves the norm but not
        // the L1 sum. With the old +90°-about-z mount the two happened to agree (it maps
        // (x,0,0) to (0,x,0)); once the mount picked up a levelling tilt they stopped, which
        // is what exposed this. What the test is for — that a negative count stays
        // negative rather than wrapping to a huge positive — the norm still checks.
        // The magnitude is what catches the sign bug: if the i16 were read as u16 the count
        // would be 64535 and this would come out near 19.7 rad/s, not 0.305. The sign of an
        // individual component is *not* assertable here — the gyro is rotated by `mount`
        // before it is returned, and that rotation is free to flip any axis.
        let magnitude: f64 = out.gyro.iter().map(|v| v * v).sum::<f64>().sqrt();
        let expected = 1000.0 * GYRO_RAD_PER_LSB;
        assert!(
            (magnitude - expected).abs() < 1e-9,
            "expected |gyro| {expected}, got {magnitude}"
        );
    }
}

#[cfg(test)]
mod freshness_tests {
    use super::*;
    #[test]
    fn tagged_identical_identity_samples_are_valid_but_invalid_quaternions_are_not() {
        let mut d = SflpDecoder::default();
        let identity = [0u8; IMU_BLOCK_LEN];
        for _ in 0..25 { assert!(d.decode_fifo(&identity).is_some()); }
        assert!(d.ready());
        let mut bad = identity;
        for v in [0x7e00u16, 0x7c00, 0x4000] { // NaN, infinity, norm too large
            bad[6..8].copy_from_slice(&v.to_le_bytes());
            assert!(d.decode_fifo(&bad).is_none());
        }
        assert!(d.decode_fifo(&identity).is_some());
    }
}
