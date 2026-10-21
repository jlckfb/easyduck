//! 主控共用控制层。
//!
//! 提供硬件接口、关节约定、观测构造、模型推理与目标保护。
//! 服务进程、socket 和 systemd 生命周期由 robotd 管理。

pub mod fall;
pub mod imu;
#[cfg(target_os = "linux")]
pub mod imu_i2c;
#[cfg(target_os = "linux")]
pub mod scs;
pub mod io;
pub mod io_trace;
pub mod model;
pub mod obs;
pub mod policy;
pub mod protection;
pub mod safety;

pub use imu::ImuData;
pub use io::{FakeIo, IoError, JointTargets, RobotIo, Sensors, SlowSensors};
pub use model::{
    BATTERY_EMPTY_V, BATTERY_FULL_V, DEFAULT_POSITION, JOINT_IDS, JOINT_NAMES, JOINT_SAFE_RANGE,
    NUM_JOINTS, battery_percent, clamp_joint, clamp_joint_targets,
};
pub use obs::{ACTION_LEN, Command, OBS_LEN, Observation};
pub use protection::{Guard, Protection, ProtectionConfig};
