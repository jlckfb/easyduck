//! 关节顺序、默认姿态、方向、目标范围与电池换算。
//! 表中数值属于既定机器人配置；不能直接视为其他机器的实物标定。

pub const NUM_JOINTS: usize = 15;

/// Servo IDs, indexed as [`JOINT_NAMES`].
pub const JOINT_IDS: [u8; NUM_JOINTS] = [
    20, 21, 22, 23, 24, // left leg
    30, 31, 32, 33, 34, // neck, head, mouth
    10, 11, 12, 13, 14, // right leg
];

/// Joint names, from the protocol crate — the wire indexes `joints` and `targets`
/// positionally, so that order and this one cannot be allowed to drift apart. The
/// assertion below is what makes "cannot" true.
pub use duck_ipc_proto::JOINT_NAMES;

const _: () = assert!(JOINT_NAMES.len() == NUM_JOINTS);

/// The mouth is absent from every alpha policy — they are all 61-D observation, 14-action,
/// and the action vector skips this index. Named so that omission is deliberate rather
/// than an off-by-one someone has to rediscover.
pub const MOUTH_INDEX: usize = 9;

/// 主控使用的默认关节姿态，单位弧度，按 JOINT_IDS 排列。
/// 观测构造以此为位置基线；实物编码器零点由驱动标定配置处理。
pub const DEFAULT_POSITION: [f64; NUM_JOINTS] = [
    0.0782,  // left_hip_yaw
    0.0614,  // left_hip_roll
    1.2164,  // left_hip_pitch  (+69.70°, 原 +80.68°)
    0.6504,  // left_knee       (+37.27°, 原 +65.27°)
    0.2347,  // left_ankle
    -0.5123, // neck_pitch
    -0.4924, // head_pitch  (-28.21°, 原 -44.30°)
    0.0337,  // head_yaw
    0.0874,  // head_roll
    0.0123,  // mouth
    -0.0430, // right_hip_yaw
    -0.0092, // right_hip_roll
    -1.4021, // right_hip_pitch (-80.33°, 原 -91.14°)
    -0.8237, // right_knee      (-47.20°, 原 -75.64°)
    -0.2945, // right_ankle
];

/// 主控模型坐标使用的关节方向约定；不是配置文件中的编码器方向。
/// 现行模型的适配关系应与观测构造和目标输出保持一致。
pub const JOINT_SIGN: [f64; NUM_JOINTS] = [
    1.0,  // left_hip_yaw    — unmeasured, assumed +1
    1.0,  // left_hip_roll   — unmeasured, assumed +1
    1.0,  // left_hip_pitch  — measured +1
    1.0,  // left_knee       — measured +1
    1.0,  // left_ankle      — measured +1
    -1.0, // neck_pitch      — measured −1 (this servo turns opposite the model)
    -1.0, // head_pitch      — measured −1 (bench, 2026-09-14)
    1.0,  // head_yaw        — unmeasured, assumed +1
    1.0,  // head_roll       — unmeasured, assumed +1
    1.0,  // mouth           — unmeasured, assumed +1
    1.0,  // right_hip_yaw   — unmeasured, assumed +1
    1.0,  // right_hip_roll  — unmeasured, assumed +1
    1.0,  // right_hip_pitch — measured +1
    1.0,  // right_knee      — measured +1
    1.0,  // right_ankle     — measured +1
];

/// Mouth travel, radians: closed and fully open. The alpha reuses the v1.6 range,
/// −5°..+30°, from `microduck_runtime`'s `variant.rs`.
///
/// The mouth is not part of any policy — every alpha network is 14 actions with this joint
/// skipped — so these two numbers and [`mouth_target`] are the whole of mouth control.
pub const MOUTH_CLOSED: f64 = -5.0 * std::f64::consts::PI / 180.0;
pub const MOUTH_OPEN: f64 = 30.0 * std::f64::consts::PI / 180.0;

/// Joint angle for a mouth opening fraction. 0 is closed, 1 is fully open; anything outside
/// is clamped rather than fed to a servo as an out-of-travel target.
pub fn mouth_target(open: f64) -> f64 {
    let open = if open.is_finite() {
        open.clamp(0.0, 1.0)
    } else {
        0.0
    };
    MOUTH_CLOSED + open * (MOUTH_OPEN - MOUTH_CLOSED)
}

/// 保留的旧总线 IMU 兼容常量，当前 I2C 采集路径不使用。
pub const IMU_DXL_ID: u8 = 200;

pub const BAUD_RATE: u32 = 1_000_000;

/// 保留的旧平台兼容寄存器表，不是 HD1910 的寄存器配置。
pub const EXPECTED_REGISTERS: &[(&str, u8)] = &[
    ("return_delay_time", 0),
    ("baud_rate", 3), // 3 = 1 Mbps, must agree with BAUD_RATE
    ("pwm_slope", 255),
    ("shutdown", 52),
];

/// Index of a joint by name. Linear scan over 15 entries, used at startup and in tests.
pub fn joint_index(name: &str) -> Option<usize> {
    JOINT_NAMES.iter().position(|n| *n == name)
}

// ── per-joint safe travel ──────────────────────────────────────────────────────────────

/// 既定机械配置的颈部俯仰参考止点，单位弧度。
/// 安全目标下界使用向内留余量的 NECK_PITCH_SAFE_LOWER。
pub const NECK_PITCH_MEASURED_STOP: f64 = -0.6461;

/// 安全钳的下界：实测止点向内留 2.5° 余量。钳子的意义是"够得着但有边界"，
/// 正好压在止点上会让每次到位都轻微顶死。
/// home 值 -0.5123 (-29.35°) 在此界内，余量 5.2°。
pub const NECK_PITCH_SAFE_LOWER: f64 = -0.6025;

pub const JOINT_SAFE_RANGE: [(f64, f64); NUM_JOINTS] = [
    (-std::f64::consts::PI, std::f64::consts::PI),              // left_hip_yaw
    (-std::f64::consts::PI, std::f64::consts::PI),              // left_hip_roll
    (-std::f64::consts::PI, std::f64::consts::PI),              // left_hip_pitch
    (-std::f64::consts::PI, std::f64::consts::PI),              // left_knee
    (-std::f64::consts::PI, std::f64::consts::PI),              // left_ankle
    (NECK_PITCH_SAFE_LOWER, std::f64::consts::PI),              // neck_pitch - 实测止点内收 2.5°
    (-std::f64::consts::PI, std::f64::consts::PI),              // head_pitch
    (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2), // head_yaw - tighter than the actuator
    (-std::f64::consts::PI, std::f64::consts::PI),              // head_roll
    (-std::f64::consts::PI, std::f64::consts::PI),              // mouth
    (-std::f64::consts::PI, std::f64::consts::PI),              // right_hip_yaw
    (-std::f64::consts::PI, std::f64::consts::PI),              // right_hip_roll
    (-std::f64::consts::PI, std::f64::consts::PI),              // right_hip_pitch
    (-std::f64::consts::PI, std::f64::consts::PI),              // right_knee
    (-std::f64::consts::PI, std::f64::consts::PI),              // right_ankle
];

/// Clamp one joint target into its travel window. `joint` is an index into
/// [`JOINT_SAFE_RANGE`]; out-of-range indices are returned unchanged rather than panicking,
/// because this sits on the path that turns an untrusted target into a servo write.
pub fn clamp_joint(joint: usize, value: f64) -> f64 {
    match JOINT_SAFE_RANGE.get(joint) {
        Some(&(lo, hi)) => value.clamp(lo, hi),
        None => value,
    }
}

/// Clamp every entry of `targets` in place; returns a bitmask of the joints that moved (bit
/// `i` = joint `i`), so the caller can report *which* joints a guard touched without a second
/// scan. NaN is treated as out-of-travel and pinned to the lower bound: a target that is not
/// a number must never reach the bus as one.
pub fn clamp_joint_targets(targets: &mut [f64; NUM_JOINTS]) -> u16 {
    let mut clamped = 0u16;
    for (i, value) in targets.iter_mut().enumerate() {
        let (lo, hi) = JOINT_SAFE_RANGE[i];
        let fixed = if value.is_nan() { lo } else { value.clamp(lo, hi) };
        if fixed != *value {
            clamped |= 1u16 << i;
            *value = fixed;
        }
    }
    clamped
}

// ── battery ──────────────────────────────────────────────────────────────────
//
// There is no fuel gauge and no ADC. The only measurement available is what the servos
// report as their own supply (`crate::bus::DynamixelIo::bus_voltage`), which is the pack
// seen through the bus — so it sags under load and recovers when the robot stands still.
// That is why the span below is *usable-under-load*, not the cell chemistry's range.

/// Off a full charge, under load. NP-F550, 2S Li-ion.
pub const BATTERY_FULL_V: f64 = 8.2;

/// The sag floor: below this the robot starts struggling, well before the pack's own
/// protection trips. Empty for our purposes, not empty for the cells'.
pub const BATTERY_EMPTY_V: f64 = 6.6;

/// Fraction of a pack, 0–100, for a bus voltage.
///
/// Linear, and the numbers come from `microduck_runtime`'s `check_battery`, where they were
/// arrived at by running a duck flat. It lives here rather than in `robotd` so there is one
/// mapping: the prototype had it in a CLI *and* re-derived in the app, which is how two
/// screens end up disagreeing about the same pack.
///
/// A non-finite or non-positive reading is 0 — those mean "no answer from the bus", and the
/// caller is expected to report that as unknown rather than to display this number.
pub fn battery_percent(volts: f64) -> f64 {
    if !volts.is_finite() || volts <= 0.0 {
        return 0.0;
    }
    ((volts - BATTERY_EMPTY_V) / (BATTERY_FULL_V - BATTERY_EMPTY_V)).clamp(0.0, 1.0) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three tables are indexed by the same integer everywhere in the crate. If they
    /// ever diverge in length, every lookup silently reads the wrong joint.
    #[test]
    fn tables_agree_on_length() {
        assert_eq!(JOINT_IDS.len(), NUM_JOINTS);
        assert_eq!(JOINT_NAMES.len(), NUM_JOINTS);
        assert_eq!(DEFAULT_POSITION.len(), NUM_JOINTS);
    }

    /// `JOINT_SIGN` is indexed like [`DEFAULT_POSITION`], so a wrong length is a silent
    /// misindex. And its one non-`+1.0` entry is the entire reason the table exists — an
    /// edit that sets `neck_pitch` back to `+1.0` should fail here, not on the robot.
    #[test]
    fn joint_sign_shape_and_the_neck_flip() {
        assert_eq!(JOINT_SIGN.len(), NUM_JOINTS);
        let neck = joint_index("neck_pitch").expect("neck_pitch is a joint");
        assert_eq!(JOINT_SIGN[neck], -1.0, "neck_pitch must be flipped");
        let head_pitch = joint_index("head_pitch").expect("head_pitch is a joint");
        assert_eq!(
            JOINT_SIGN[head_pitch], -1.0,
            "head_pitch must be flipped (measured 2026-09-14): the hardware counts RISE \
             when the head is pushed up, while the model's qpos+ pitches it DOWN, and \
             hand-levelling puts its home 16.08 deg above DEFAULT_POSITION[6]"
        );
        for (i, &s) in JOINT_SIGN.iter().enumerate() {
            if i == neck || i == head_pitch {
                continue;
            }
            assert_eq!(s, 1.0, "joint {i} ({}) should be +1", JOINT_NAMES[i]);
        }
    }

    /// `head_pitch` is the second flipped joint, measured 2026-09-14 with the torque-off
    /// hand-push method (all 15 servos released, operator pushes the beak, encoder counts
    /// read back).
    ///
    /// Three things put it at `-1.0`:
    /// 1. The control group passed. The same method on `neck_pitch`, whose sign is already
    ///    known, gave +2094 counts for a downward push against a previously measured +2100,
    ///    so the method reproduces a known answer.
    /// 2. The model agrees. `head_pitch` qpos +0.15 rad moves the beak site `mouth_tip`
    ///    forward +3.08 mm and DOWN -9.22 mm, i.e. the model's positive direction is
    ///    *looking down*, while the hardware's rising counts are *looking up*.
    /// 3. The live robot agrees. Streaming `robot.head{head_pitch = -0.4924}` at 31.5 Hz
    ///    drove `targets[6]` to -59.57/-61.24/-61.34/-61.66 deg -- exactly the
    ///    `-44.30 - 16.08 = -60.38` an inverted `+16.08 deg` request predicts.
    ///
    /// The home pose was re-measured in the same sitting: a hand-levelled head reads
    /// -0.4924 rad, i.e. 16.08 deg above the old `-0.7731`, which is the droop the operator
    /// had been reporting by eye. Setting this entry back to `+1.0` should fail here rather
    /// than on the robot.
    #[test]
    fn head_pitch_must_be_flipped() {
        let i = joint_index("head_pitch").expect("head_pitch is a joint");
        assert_eq!(JOINT_SIGN[i], -1.0, "head_pitch must be flipped");
        assert!(
            (DEFAULT_POSITION[i] + 0.4924).abs() < 1e-9,
            "head_pitch home should be the hand-levelled -0.4924 rad, got {}",
            DEFAULT_POSITION[i]
        );
    }

    /// A duplicated Dynamixel ID makes a `sync_read` return blocks that cannot be matched
    /// back to joints, and a `sync_write` command two joints at once. Both fail in ways
    /// that look like a wiring fault.
    #[test]
    fn ids_are_unique() {
        let mut seen = JOINT_IDS;
        seen.sort_unstable();
        seen.windows(2)
            .for_each(|w| assert_ne!(w[0], w[1], "duplicate Dynamixel ID {}", w[0]));
    }

    /// The IMU board shares the bus with the servos, so its ID must not collide with one.
    #[test]
    fn imu_id_does_not_collide_with_a_joint() {
        assert!(!JOINT_IDS.contains(&IMU_DXL_ID));
    }

    /// `MOUTH_INDEX` is used to skip a slot when mapping 14 policy actions onto 15 joints.
    /// Pointing it at the wrong joint would shift every action after it by one.
    #[test]
    fn mouth_index_names_the_mouth() {
        assert_eq!(JOINT_NAMES[MOUTH_INDEX], "mouth");
        assert_eq!(joint_index("mouth"), Some(MOUTH_INDEX));
    }

    /// The ends of the span and the middle of it. Getting the direction wrong here would
    /// report a full pack as flat, which is the kind of thing nobody double-checks.
    #[test]
    fn battery_percent_spans_the_usable_range() {
        assert_eq!(battery_percent(BATTERY_FULL_V), 100.0);
        assert_eq!(battery_percent(BATTERY_EMPTY_V), 0.0);
        assert!((battery_percent(7.4) - 50.0).abs() < 0.001);
    }

    /// Voltages outside the span are ordinary — a fresh pack reads over 8.2 V off the
    /// charger, and a robot being run into the ground reads under 6.6 V. Neither may
    /// produce a percentage outside 0–100 for a caller to display.
    #[test]
    fn battery_percent_clamps_rather_than_extrapolating() {
        assert_eq!(battery_percent(9.5), 100.0);
        assert_eq!(battery_percent(5.0), 0.0);
    }

    /// A bus that did not answer arrives here as 0.0, and NaN is what a mean over an empty
    /// set produces. Both must be 0 rather than a wild number a caller might print.
    #[test]
    fn battery_percent_treats_no_reading_as_zero() {
        assert_eq!(battery_percent(0.0), 0.0);
        assert_eq!(battery_percent(f64::NAN), 0.0);
        assert_eq!(battery_percent(-1.0), 0.0);
    }

    /// The mouth range is the prototype's: −5° closed, +30° open. A fraction outside 0..1
    /// (or a NaN from a broken client) must clamp rather than command a servo past travel.
    #[test]
    fn mouth_target_spans_the_prototype_range() {
        assert!((mouth_target(0.0) - (-5.0f64.to_radians())).abs() < 1e-12);
        assert!((mouth_target(1.0) - 30.0f64.to_radians()).abs() < 1e-12);
        assert_eq!(mouth_target(-3.0), mouth_target(0.0));
        assert_eq!(mouth_target(7.0), mouth_target(1.0));
        assert_eq!(mouth_target(f64::NAN), mouth_target(0.0));
    }

    /// The legs are *close to* mirrored: the roll/pitch/ankle pairs nearly cancel. A sign
    /// typo in the home pose is invisible by inspection and makes the robot stand crooked,
    /// which is what this is for — a flipped sign is off by twice the value, not by the few
    /// degrees of assembly tolerance below.
    ///
    /// It is deliberately not exact. `DEFAULT_POSITION` is a *bench table* (see the note
    /// above it): each leg was posed and read on its own, and the two do not come back
    /// equal. On the 2026-09-12 calibration the residuals were ~0.18 rad on hip_pitch and
    /// knee against a tolerance of 0.25 rad here. Tightening this to zero would mean
    /// overwriting the measured pose of one leg with the other's, which is a worse robot,
    /// not a more correct table.
    #[test]
    fn home_pose_legs_are_mirrored() {
        const TOL: f64 = 0.25;
        for (left, right) in [
            ("left_hip_roll", "right_hip_roll"),
            ("left_hip_pitch", "right_hip_pitch"),
            ("left_knee", "right_knee"),
            ("left_ankle", "right_ankle"),
        ] {
            let l = DEFAULT_POSITION[joint_index(left).unwrap()];
            let r = DEFAULT_POSITION[joint_index(right).unwrap()];
            assert!(
                (l + r).abs() < TOL,
                "{left} ({l}) and {right} ({r}) should be near-opposite within {TOL}"
            );
        }
    }
}
