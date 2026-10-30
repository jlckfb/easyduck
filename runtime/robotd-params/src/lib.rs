//! robotd 配置结构、默认值和校验。
//!
//! 主控在启动时读取配置，修改后需重启。
//! 参数登记表供命令行配置工具复用，避免维护不同的参数定义。

pub mod registry;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where a release is mounted. Policy paths default under here, so an ordinary update
/// carries the policy with the binaries that were trained against it.
pub const RELEASE_DIR: &str = "/opt/robot/daemon/current";

/// Where a provisioned robot keeps it, alongside the updater's own config.
pub const DEFAULT_PATH: &str = "/etc/robot/robotd.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Params {
    pub bus: Bus,
    pub control: Control,
    pub imu: Imu,
    pub update_gate: UpdateGate,
    pub policy: PolicyParams,
    pub safety: SafetyParams,
    pub audio: AudioParams,
    pub chorale: ChoraleParams,
    pub media: MediaParams,
    pub detect: DetectParams,
}

/// Where orientation comes from, and how to reach it.
///
/// The bus board and the I²C chip are mutually exclusive by construction: a robot carries
/// one or the other. `source` decides which, and the loser of that choice is simply never
/// opened — an I²C robot has no bus board on the wire, and a bus robot has no chip wired up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Imu {
    /// Registered I2C chip. Omitted legacy configs mean LSM6DSV16X.
    pub model: String,
    /// `"bus"`: the IMU board riding the Dynamixel bus (the original design).
    /// `"i2c"`: the selected IMU wired to an I²C bus, read directly by this process.
    pub source: String,
    /// I²C bus device when `source = "i2c"`; the deployed path is configuration, not a chip default.
    pub i2c_dev: String,
    /// 7-bit address: LSM 0x6A/0x6B, BNO085 0x4A/0x4B.
    pub i2c_addr: u8,
    /// Orientation pipeline on the I²C chip:
    /// LSM: `"sflp"` chip fusion or `"comp"` host complementary fusion.
    /// BNO085: `"game_rv"` chip fusion; LSM-specific modes are rejected.
    pub fusion: String,
    /// Lever arm: IMU position relative to the trunk origin, in the trunk frame (metres).
    /// Used by the `"comp"` fusion to subtract ω×(ω×r)+α×r from the accelerometer.
    pub lever_arm: [f64; 3],
    /// Complementary-fusion correction gain per tick (accelerometer pulls the gyro-propagated
    /// gravity estimate toward the measured direction). 0.05 @50 Hz ≈ 0.6 s time constant.
    pub g_filter_k: f64,
    /// When |accel| departs from 1 g by more than this (in g), the accelerometer is treated
    /// as impact-contaminated and its correction weight is driven to zero for that tick.
    pub accel_trust_g: f64,
}

impl Default for Imu {
    fn default() -> Self {
        Self {
            source: "bus".into(),
            model: "lsm6dsv16x".into(),
            i2c_dev: "/dev/i2c-3".into(),
            i2c_addr: 0x6A,
            fusion: "sflp".into(),
            lever_arm: [-0.014, 0.001, 0.0567],
            g_filter_k: 0.05,
            accel_trust_g: 0.3,
        }
    }
}

/// The one video mode a robot streams in, as a name rather than four numbers.
///
/// **Frame size, rate and a matching bitrate move together or not at all.** They are not
/// independent settings: 1080p at the 2 Mb/s that suits 720p is a smear, and 720p at 6 Mb/s
/// spends a link's headroom on nothing. Offering `width`, `height`, `fps` and `bitrate` as four
/// keys would make every wrong combination of them expressible — including the ones the capture
/// path cannot produce at all, and a pipeline that will not start costs the WebRTC *control*
/// channel along with the video, because the two are bundled (`remote-webrtc.md`).
///
/// So the ladder is fixed, and every rung is 16:9 — the sensor's own aspect. A mode that changed
/// the shape of the picture would be cropping or squashing rather than lowering quality, which is
/// not what anybody picking "smaller" is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Quality {
    /// The sensor's full frame. The most detail, and the rung least likely to hold 30 fps on
    /// this ISP path — [`MediaParams`] says what is measured and what is not.
    #[serde(rename = "1080p30")]
    Q1080p30,
    /// What every measurement in `mediad` was taken at, and the default.
    #[default]
    #[serde(rename = "720p30")]
    Q720p30,
    /// Same picture, half the frames: the rung for a link that cannot carry 30.
    #[serde(rename = "720p15")]
    Q720p15,
    /// Small and cheap, for a bad link or a busy CPU.
    #[serde(rename = "360p30")]
    Q360p30,
}

/// Every mode, in the order an editor cycles them — and the strings the file uses.
///
/// One list, so the registry's choices, the file's values and [`Quality`] itself cannot disagree;
/// [`tests::every_quality_label_round_trips`] pins it to the enum in both directions.
pub const QUALITY_LABELS: &[&str] = &["1080p30", "720p30", "720p15", "360p30"];

impl Quality {
    /// The modes, in [`QUALITY_LABELS`] order.
    pub const ALL: [Quality; 4] = [
        Quality::Q1080p30,
        Quality::Q720p30,
        Quality::Q720p15,
        Quality::Q360p30,
    ];

    /// The name this mode has in the file.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Q1080p30 => "1080p30",
            Quality::Q720p30 => "720p30",
            Quality::Q720p15 => "720p15",
            Quality::Q360p30 => "360p30",
        }
    }

    /// Frame size in pixels. Every rung is 16:9 and every dimension is a multiple of 8, which
    /// is what the ISP's scaler and the encoder's macroblocks both want.
    pub fn size(self) -> (u32, u32) {
        match self {
            Quality::Q1080p30 => (1920, 1080),
            Quality::Q720p30 | Quality::Q720p15 => (1280, 720),
            Quality::Q360p30 => (640, 360),
        }
    }

    pub fn width(self) -> u32 {
        self.size().0
    }

    pub fn height(self) -> u32 {
        self.size().1
    }

    pub fn fps(self) -> u32 {
        match self {
            Quality::Q720p15 => 15,
            _ => 30,
        }
    }

    /// What this mode streams at when `[media] bitrate` is unset — bits per second.
    ///
    /// Scaled with the pixel rate rather than picked per rung: 720p30 is the measured 2 Mb/s
    /// `mediad` has always used, and the others are that number times their share of the pixels
    /// per second, rounded to something a human can read. Congestion control moves from here, so
    /// this is a starting point rather than a cap.
    pub fn default_bitrate(self) -> u32 {
        match self {
            Quality::Q1080p30 => 4_000_000,
            Quality::Q720p30 => 2_000_000,
            Quality::Q720p15 => 1_000_000,
            Quality::Q360p30 => 800_000,
        }
    }
}

/// How `mediad` decides what bitrate to actually send at.
///
/// **This is a CPU setting as much as a network one.** The estimator is not free: on the board,
/// with one peer connected, `rtpgccbwe` is the single largest consumer in the process — 7.6% of a
/// core against `v4l2src`'s 0.3% — because it works per packet while capture works per DMABuf
/// handle. Turning it off deletes that thread.
///
/// What it costs is adaptivity, and that is not a small thing: adapting the rate to the link is
/// the whole reason `webrtcsink` is handed raw video rather than pre-encoded H.264
/// (`mediad::pipeline`). On a link that stays good — a robot one hop away on its own LAN — the
/// estimator spends CPU discovering a ceiling it will never hit. On a link that degrades, it is
/// what keeps a picture rather than a stall.
///
/// **It also decides what `bitrate` means.** With an estimator running, `bitrate` is a starting
/// point it ramps away from within seconds. Disabled, nothing moves it, and `bitrate` is the rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CongestionControl {
    /// Nothing adapts. `bitrate` is exactly what is sent, and a link that degrades degrades the
    /// picture rather than the rate.
    Disabled,
    /// `webrtcsink`'s own sender-side heuristic. Cheaper than the estimator, blunter than it.
    Homegrown,
    /// Google Congestion Control, and `webrtcsink`'s own default — so this is what every robot has
    /// been running, and naming it here changes nothing.
    #[default]
    Gcc,
}

/// Every mode, in the order an editor cycles them.
///
/// These are `webrtcsink`'s own property nicknames rather than names of ours: they are what gets
/// set on the element, and a second vocabulary in between would be one more thing to get wrong.
/// Note `gcc`, not `googcc` — [`tests::every_congestion_label_round_trips`] pins the spelling.
pub const CONGESTION_LABELS: &[&str] = &["disabled", "homegrown", "gcc"];

impl CongestionControl {
    pub const ALL: [CongestionControl; 3] = [
        CongestionControl::Disabled,
        CongestionControl::Homegrown,
        CongestionControl::Gcc,
    ];

    /// The `congestion-control` nickname `webrtcsink` knows this by.
    pub fn nick(self) -> &'static str {
        match self {
            CongestionControl::Disabled => "disabled",
            CongestionControl::Homegrown => "homegrown",
            CongestionControl::Gcc => "gcc",
        }
    }
}

/// `[media]` — what `mediad` streams.
///
/// **These were command-line flags in `mediad.service`, and that is why this section exists.**
/// The release installer rewrites that unit file, so the only supported way to change a flag was
/// a systemd drop-in — a mechanism nobody reaches for to answer "why is the video soft?". Here
/// they are three keys in the file `robotctl configure` already edits.
///
/// `mediad` reads this file at startup and nothing else does anything to it, so a change needs
/// `systemctl restart mediad` — not `robotd`. The editor offers the right one.
///
/// **What is measured and what is not.** 720p30 is the rung every number in `mediad::pipeline`
/// comes from: 29.3 fps off the ISP main path, with the capture format and buffer depth that took
/// three bench sessions to find. The sensor is pinned to a 1920x1080 mode that runs at 30 and the
/// ISP scales down from it, so 1080p30 asks for no scaling at all — what is unmeasured there is
/// whether the capture path and the encoder hold 30 fps at 2.25x the pixels. A rung that does not
/// hold runs slower; it is not a pipeline that fails to start.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MediaParams {
    /// Stream the head camera. `false` streams a test pattern instead, which is what a board
    /// with no camera wants: the pipeline starts, so the WebRTC control channel exists.
    pub camera: bool,
    /// Frame size and rate, as one name. [`Quality`] says why it is one key and not four.
    pub quality: Quality,
    /// Starting video bitrate, bits per second. Unset follows the quality —
    /// [`Quality::default_bitrate`] — which is what almost every robot wants.
    ///
    /// A *starting* point unless `congestion_control` is `disabled`, which is the one setting that
    /// makes this the rate.
    pub bitrate: Option<u32>,
    /// Whether the send rate adapts to the link, and by what. [`CongestionControl`] has the
    /// trade — it is the largest single CPU consumer in this process.
    pub congestion_control: CongestionControl,
}

impl Default for MediaParams {
    fn default() -> Self {
        Self {
            // On, because a robot with a camera is the case, and a board without one shows a
            // test pattern rather than nothing only if somebody turns this off.
            camera: true,
            quality: Quality::default(),
            bitrate: None,
            // `webrtcsink`'s own default, named rather than inherited: what the element defaults
            // to is a fact about a plugin we ship from a pinned release, and the day it changes
            // should not be the day every robot's send rate changes with it.
            congestion_control: CongestionControl::default(),
        }
    }
}

/// `[detect]` — finding other ducks in the camera.
///
/// **Read by `mediad`, not by `robotd`**, which is a first for this file: the frames are on
/// `mediad`'s tee and perception belongs next to the sensor. It lives here anyway, because this is
/// the file `robotctl configure` edits and a robot has one place where its switches are — a second
/// config file for the second daemon that wants one is how a fleet ends up with settings nobody can
/// find.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DetectParams {
    /// Off by default. The detector costs a model in the release, ~50 ms of CPU per frame and some
    /// heat; a robot that nothing asks to look for ducks should not be paying for it.
    pub enabled: bool,
    /// Where to look, and therefore *what runs it*: a `.rknn` goes to the NPU, an `.onnx` runs on
    /// the CPU. Absent means the release's own model, NPU first — see [`DetectParams::model`].
    pub model: Option<PathBuf>,
    /// Frames per second to run the detector at.
    ///
    /// **2 Hz is a thermal number, not a taste.** Flat out on a Radxa Zero 3 this reaches 95 °C and
    /// the CPU throttles to 408 MHz, which is a robot that walks badly to see well. Two looks a
    /// second is plenty for "is there a duck over there", and costs about a tenth of one core.
    pub hz: f64,
    /// Confidence a detection needs, against **this** model.
    ///
    /// A quantised model's output tensor carries its own scale, so the number that means 0.9 on the
    /// float model does not mean 0.9 here — the INT8 scores of the shipped model saturate around
    /// 1.4. Tuned on the board, not inherited from training.
    pub threshold: f32,
}

impl Default for DetectParams {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            hz: 2.0,
            threshold: 0.35,
        }
    }
}

impl MediaParams {
    /// The bitrate the daemon will actually start at.
    pub fn bitrate_resolved(&self) -> u32 {
        self.bitrate
            .unwrap_or_else(|| self.quality.default_bitrate())
    }
}

impl DetectParams {
    /// The models to try, best first. Empty when the detector is off.
    ///
    /// **A list, not a choice**, because whether the NPU works is not something this file can know.
    /// The `.rknn` is preferred — it is why the detector is cheap — but a board whose NPU is
    /// disabled in its device tree (which is how Armbian ships the Radxa Zero 3) or which never ran
    /// `setup-npu.sh` has no runtime to load it with. Falling through to the `.onnx` means such a
    /// board still sees, on the CPU, instead of logging one warning and doing nothing for ever.
    ///
    /// An explicit `model` is the operator being specific, so it is tried alone.
    pub fn models(&self) -> Vec<PathBuf> {
        if !self.enabled {
            return Vec::new();
        }
        if let Some(path) = &self.model {
            if is_none_sentinel(path) {
                return Vec::new();
            }
            return vec![path.clone()];
        }
        let release = PathBuf::from(RELEASE_DIR).join("models");
        [
            release.join("duck_detect.rknn"),
            release.join("duck_detect.onnx"),
        ]
        .into_iter()
        .filter(|path| path.exists())
        .collect()
    }
}

/// `[chorale]` — several ducks singing one piece.
///
/// `accept` is **false by default, and that is the whole section.** A chorale is not only a sound:
/// it moves the mouth and it moves the head. A robot that began animating because another robot
/// walked into the room would be doing motion nobody asked for, in someone's living room, and two
/// people's ducks in a café have no business pairing up. Off also means *invisible* rather than
/// visibly declining — a duck that has not opted in puts nothing on the air at all.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ChoraleParams {
    /// Whether this robot may sing with others at all. `false` — and it is derived rather than
    /// written out, so that the default cannot be changed by editing one word.
    pub accept: bool,
}

/// `[audio]` — audio playback. All optional equipment: a robot without a
/// codec (or a bank) walks identically and stays quiet, so nothing here reaches a health
/// verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AudioParams {
    /// Master switch for audio playback.
    pub enabled: bool,
    /// ALSA playback device — the TLV320AIC3104 codec.
    pub device: String,
    /// Where the per-robot voice bank lives. The release's postinstall renders it there
    /// (`sounds ensure-bank`), seeded from the SoC serial.
    pub bank: PathBuf,
    /// Quack once as the control loop comes up. On by default because on a headless board
    /// it is the audible "robotd is running"; off for anyone who restarts the daemon all
    /// day and would rather it did so quietly.
    pub greet: bool,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            enabled: true,
            device: "plughw:aic3104".to_owned(),
            bank: PathBuf::from("/var/lib/robot/sounds"),
            greet: true,
        }
    }
}

/// Which drive configuration this robot runs. One robot, two personalities: legs, or the
/// roller. They differ in policies *and* tuning, so the mode is one switch here rather than
/// six paths an operator has to keep consistent — the prototype's launcher kept two whole
/// command lines for the same reason. Switching is an edit plus `systemctl restart robotd`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Walk,
    Roller,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Walk => "walk",
            Mode::Roller => "roller",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PolicyParams {
    /// Whether to load a policy at all.
    ///
    /// False means slice 1's behaviour: run the loop, hold the pose, stay healthy. That is a
    /// legitimate configuration — it is the safest thing to be doing while hammering
    /// install/rollback cycles at a bench — and it is distinct from a policy that was wanted
    /// and could not be loaded, which is unhealthy.
    pub enabled: bool,
    /// `walk` (default) or `roller`. Changes which policies load *and* the tuning defaults
    /// below — every unset field resolves per mode, so a roller robot needs one line.
    pub mode: Mode,
    /// Policy paths. Absent means the mode's default inside the release directory, so a
    /// normal update ships them; point one elsewhere to try a build without cutting a
    /// release. The literal `"none"` disables a slot outright — the prototype's convention.
    pub walk: Option<PathBuf>,
    /// Standing policy. Without one the walking policy runs at every velocity.
    pub stand: Option<PathBuf>,
    /// Commanded sit↔stand (posture flag in the twist `vx` slot). Sit toggle, shutdown sit
    /// and the seated-boot rise all need it.
    pub sitstand: Option<PathBuf>,
    /// Optional commanded sit/stand for xgoduck; sitstand remains recovery.
    pub posture: Option<PathBuf>,
    /// Phase-scripted ground pick. In roller mode this slot holds the crouch.
    pub ground_pick: Option<PathBuf>,
    pub kick_left: Option<PathBuf>,
    pub kick_right: Option<PathBuf>,
    /// Episodic forward roll. Ships by default in both modes, as the prototype now does.
    pub roulade: Option<PathBuf>,
    /// Scales raw policy output into a joint offset. Absent resolves per mode: 0.9 walking
    /// (the prototype's alpha default), 0.8 roller.
    pub action_scale: Option<f64>,
    pub standing_action_scale: f64,
    /// Standing runs softer, at this fraction of `gain`.
    pub standing_gain_ratio: f64,
    /// Position P gain while running.
    pub gain: u16,
    /// Servo-internal Position I gain, register 82. Default 0 — what the daemon pinned before
    /// this was a knob. Closes standing sag that P is too soft to finish; winds up on a joint
    /// held off its target by a stop, so raise it in small steps.
    pub gain_i: u16,
    /// Servo-internal Position D gain, register 80. Default 0, as pinned historically.
    ///
    /// The only knob not on the same axis as `gain`/`action_scale`: those scale with error and
    /// so move the stiction breakaway threshold and the lurch after it together. D scales with
    /// error *rate* — zero at rest, large mid-lurch. Costs torque chatter, since it
    /// differentiates encoder quantisation (0.0879 deg/count).
    pub gain_d: u16,
    /// First-order low-pass on the head joint targets, `1.0` = pass-through. Default 0.5
    /// in both modes — the value the alpha policies are *trained* with, so it must match
    /// or transfer degrades. (The roller preset used to ship it off; the prototype rebased
    /// its roller line on the alpha defaults, and this follows.)
    pub head_lowpass: Option<f64>,
    /// Same, for the ten leg joints. Walking default 0.7.
    pub legs_lowpass: Option<f64>,
    /// One ground-pick cycle, seconds. The move ends at 70% of the cycle, as the prototype
    /// does. Absent resolves per mode: 4.0 walking, 3.0 roller (the crouch).
    pub ground_pick_period: Option<f64>,
    /// Action scale while the ground pick runs. Absent: 1.0 walking, 0.8 roller.
    pub ground_pick_action_scale: Option<f64>,
    /// Gain multiplier while the ground pick runs.
    pub ground_pick_gain_ratio: f64,
    /// How long a kick window stays on the kick network, seconds.
    pub kick_duration: f64,
    /// One roulade — one forward roll, seconds. Holding the button chains rolls; this is
    /// the length of each. The prototype's measured single-roll time.
    pub roulade_duration: f64,
    /// Action scale while a roulade runs.
    pub roulade_action_scale: f64,
    /// Gain multiplier while a roulade runs.
    pub roulade_gain_ratio: f64,
    /// Scale actions with battery voltage: effective scale × (nominal / measured). The
    /// servos' effective kP tracks their supply, so this holds the robot's response steady
    /// as the pack sags. Off by default, as in the prototype.
    pub voltage_adapt: bool,
    /// Reference voltage for `voltage_adapt` — the supply the gains were identified at.
    pub nominal_voltage: f64,
}

/// The literal that disables an optional policy slot, per the prototype's `--x-policy None`.
fn is_none_sentinel(path: &std::path::Path) -> bool {
    path.as_os_str().eq_ignore_ascii_case("none")
}

/// `[policy]` with every absent field resolved against the mode's defaults.
///
/// This is what the rest of `robotd` consumes — nothing downstream should ever have to ask
/// "walk or roller?" to know the action scale.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPolicy {
    pub enabled: bool,
    pub mode: Mode,
    pub walk: PathBuf,
    pub stand: Option<PathBuf>,
    pub sitstand: Option<PathBuf>,
    /// Optional commanded sit/stand for xgoduck; sitstand remains recovery.
    pub posture: Option<PathBuf>,
    pub ground_pick: Option<PathBuf>,
    pub kick_left: Option<PathBuf>,
    pub kick_right: Option<PathBuf>,
    pub roulade: Option<PathBuf>,
    pub action_scale: f64,
    pub standing_action_scale: f64,
    pub standing_gain_ratio: f64,
    pub gain: u16,
    /// Servo-internal Position I gain, register 82. Default 0 — what the daemon pinned before
    /// this was a knob. Closes standing sag that P is too soft to finish; winds up on a joint
    /// held off its target by a stop, so raise it in small steps.
    pub gain_i: u16,
    /// Servo-internal Position D gain, register 80. Default 0, as pinned historically.
    ///
    /// The only knob not on the same axis as `gain`/`action_scale`: those scale with error and
    /// so move the stiction breakaway threshold and the lurch after it together. D scales with
    /// error *rate* — zero at rest, large mid-lurch. Costs torque chatter, since it
    /// differentiates encoder quantisation (0.0879 deg/count).
    pub gain_d: u16,
    pub head_lowpass: Option<f64>,
    pub legs_lowpass: Option<f64>,
    pub ground_pick_period: f64,
    pub ground_pick_action_scale: f64,
    pub ground_pick_gain_ratio: f64,
    pub kick_duration: f64,
    pub roulade_duration: f64,
    pub roulade_action_scale: f64,
    pub roulade_gain_ratio: f64,
    pub voltage_adapt: bool,
    pub nominal_voltage: f64,
}

impl PolicyParams {
    pub fn resolved(&self) -> ResolvedPolicy {
        let release = |name: &str| PathBuf::from(RELEASE_DIR).join("policies").join(name);
        let path = |field: &Option<PathBuf>, default: Option<&str>| -> Option<PathBuf> {
            match field {
                Some(p) if is_none_sentinel(p) => None,
                Some(p) => Some(p.clone()),
                None => default.map(release),
            }
        };

        let (walk_default, stand, sitstand, ground_pick, kick) = match self.mode {
            Mode::Walk => (
                "alpha_walking.onnx",
                Some("alpha_stand.onnx"),
                Some("alpha_sitstand.onnx"),
                Some("alpha_ground_pick.onnx"),
                true,
            ),
            // The prototype's roller preset, since rebased on the alpha defaults: roller
            // policy, crouch on the ground-pick trigger, and everything else — sit/stand,
            // kicks, the trained low-pass — as the walking mode has it. `stand` stays
            // unloaded, deliberately: the prototype loads the standing network in roller
            // mode and then skips every standing transition while `roller_mode` is set, so
            // it never runs — not loading it is the same robot without the dead session.
            Mode::Roller => (
                "roller.onnx",
                None,
                Some("alpha_sitstand.onnx"),
                Some("roller_crouch.onnx"),
                true,
            ),
        };

        ResolvedPolicy {
            enabled: self.enabled,
            mode: self.mode,
            walk: path(&self.walk, Some(walk_default)).expect("walk always has a default"),
            stand: path(&self.stand, stand),
            sitstand: path(&self.sitstand, sitstand),
            posture: path(&self.posture, None),
            ground_pick: path(&self.ground_pick, ground_pick),
            kick_left: path(&self.kick_left, kick.then_some("ball_kick_left.onnx")),
            kick_right: path(&self.kick_right, kick.then_some("ball_kick_right.onnx")),
            roulade: path(&self.roulade, Some("roulade.onnx")),
            action_scale: self.action_scale.unwrap_or(match self.mode {
                Mode::Walk => 0.9,
                Mode::Roller => 0.8,
            }),
            standing_action_scale: self.standing_action_scale,
            standing_gain_ratio: self.standing_gain_ratio,
            gain: self.gain,
            gain_i: self.gain_i,
            gain_d: self.gain_d,
            head_lowpass: Some(self.head_lowpass.unwrap_or(0.5)).filter(|a| *a < 1.0),
            legs_lowpass: Some(self.legs_lowpass.unwrap_or(0.7)).filter(|a| *a < 1.0),
            ground_pick_period: self.ground_pick_period.unwrap_or(match self.mode {
                Mode::Walk => 4.0,
                Mode::Roller => 3.0,
            }),
            ground_pick_action_scale: self.ground_pick_action_scale.unwrap_or(match self.mode {
                Mode::Walk => 1.0,
                Mode::Roller => 0.8,
            }),
            ground_pick_gain_ratio: self.ground_pick_gain_ratio,
            kick_duration: self.kick_duration,
            roulade_duration: self.roulade_duration,
            roulade_action_scale: self.roulade_action_scale,
            roulade_gain_ratio: self.roulade_gain_ratio,
            voltage_adapt: self.voltage_adapt,
            nominal_voltage: self.nominal_voltage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SafetyParams {
    /// Projected-gravity z above which the robot counts as going down. Upright is about
    /// -1.0; on its side is near 0.
    pub fall_gravity_z: f64,
    /// How long that has to hold. Debounced so a firm footfall is not a fall.
    pub fall_debounce_ms: u64,
    /// Intent age past which the velocity is zeroed. Stop, not limp.
    pub deadman_ms: u64,
    /// The gain limp-fall yields at — low enough to give way rather than fight the floor.
    pub gain_limp: u16,
    /// Stop driving the joints once the fall verdict holds, and command the pose the robot
    /// is already in at [`Self::gain_limp`] until it is upright again.
    ///
    /// **Off by default, which is upstream behaviour**: a fall is a report, not a preemption
    /// (`duck-control/src/safety.rs` module docs), and the humans stay in charge.
    ///
    /// Turn it on when the `stand` slot is disabled. `limp_fall` hands the robot back to
    /// "the standing policy" after posing (`robotd/src/main.rs`, `posed — handing back`),
    /// but with `stand = "none"` there is no standing network to hand it to, so the walking
    /// one gets a robot lying on its side. Its observation is nothing it was trained on, and
    /// its output is driven into the joints at the full running gain: measured on the alpha
    /// unit 2026-09-15, `walk` at gain 200 while `projected_gravity` said 106°.
    ///
    /// Un-latches on its own. The verdict is debounced in both directions, so picking the
    /// robot up clears it and the policy resumes — no reset, no `init`.
    pub fall_stops_driving: bool,
    /// Sit down and power the machine off when the battery EMA reaches the empty floor
    /// (6.6 V — `duck_control::model::BATTERY_EMPTY_V`). The EMA moves over ~10 s, so a
    /// load sag cannot trip it.
    pub battery_empty_shutdown: bool,
    /// Sit and warn once without powering off. Latched until robotd restarts.
    pub low_battery_sit: bool,
    /// Trigger on the smoothed motor-bus voltage (2S pack), inclusive.
    pub low_battery_voltage: f64,

    /// Go limp *while falling*, to land soft instead of fighting the floor all the way
    /// down. **On by default** since it was validated on a robot — the whole point is that
    /// the fleet lands soft, and a mode every board has to opt into individually is a mode
    /// most boards do not have.
    ///
    /// The only thing the daemon does about a fall. Drop to `gain_limp`, let the robot
    /// collapse, pose it back to standing once it has landed, then hand it to the standing
    /// policy — which stands up far more cleanly from a still robot than from one that has
    /// been thrashing since the fall began. With it off, a fall changes nothing: the policy
    /// keeps driving and the humans stay in charge.
    pub limp_fall: bool,
    /// Projected-gravity z the robot must already be past before a fall prediction counts
    /// — about 26° of tilt, which ordinary walking does not reach.
    pub limp_fall_tilt_z: f64,
    /// Where the extrapolation must reach to count as falling. Same sense as
    /// `fall_gravity_z`, and by default the same number.
    pub limp_fall_predict_z: f64,
    /// How far ahead the tilt rate is extrapolated.
    pub limp_fall_lookahead_ms: u64,
    /// How long the fall verdict must hold before the gains drop. Three ticks at 50 Hz —
    /// longer than a footfall impulse, short enough to leave most of the fall to limp
    /// through.
    pub limp_fall_debounce_ms: u64,
    /// Angular-rate magnitude below which the robot counts as having landed, rad/s.
    pub limp_fall_still_rate: f64,
    /// How long it has to stay that still before the limp ends.
    pub limp_fall_still_ms: u64,
    /// Hard cap on the limp, however the landing reads. A robot that never goes still —
    /// held in someone's hands, or resting against something that keeps nudging it —
    /// must not stay limp forever.
    pub limp_fall_max_ms: u64,
    /// How long the ramp back to the standing pose takes, once the robot has landed.
    /// 0.6 s — settled on at the robot. The joints travel across the floor unloaded rather
    /// than lifting anything, so a full second was mostly dead time before the stand-up;
    /// 0.6 keeps some margin over the 0.3 that also worked.
    pub limp_fall_pose_ms: u64,
    /// Gain for that ramp. The joints have to actually travel across the floor, so it is
    /// not the limp gain; it is the softened standing gain rather than the walking one.
    pub limp_fall_pose_gain: u16,

    // ── per-joint protection (A-7) ───────────────────────────────────────────
    //
    // Three guards against the three ways a servo dies, all of them in `duck_control`'s
    // `protection` module rather than here: this file says what a *robot* wants, and the
    // reasoning for each number lives with the code that applies it.
    //
    // The accident they answer: a policy commanded `neck_pitch` to −117.6° — this robot's
    // home pose for that joint — and the servo could only reach −48°. It sat 69° from its
    // target, pushing, at walking gain, until the case read 55 °C and someone unplugged the
    // bus. Nothing in the runtime had a per-joint limit, a current limit, or a thermal
    // limit; a joint could stall indefinitely.
    /// Hold every target inside its joint's safe travel (`duck_control::model::JOINT_SAFE_RANGE`).
    ///
    /// **On by default.** Turning it off restores the old behaviour — the actuator's ±π and
    /// nothing else — which is what a bench A/B wants and what a robot should never run.
    pub joint_limit_clamp: bool,

    /// Detect a joint that is pushing without moving, and hold it where it is until it is
    /// driven again.
    pub stall_protection: bool,

    /// Soften a joint that is over its thermal threshold.
    pub overtemp_protection: bool,

    /// The standing error that counts as *pushing*, degrees. An error under this is tracking
    /// lag, not a blocked joint.
    pub stall_error_deg: f64,

    /// Present current that counts as *loaded*, mA — the first of the two load signals.
    pub stall_current_ma: f64,

    /// `|present PWM|` that counts as *loaded*, in the servo's own counts (885 is full).
    ///
    /// The second load signal, and not a duplicate of the first: the current *sensor* is the
    /// part of a clone servo least likely to be calibrated, and a guard that needs one
    /// specific reading to fire is a guard that silently never fires on the board that needs
    /// it. Either signal is enough.
    pub stall_pwm: f64,

    /// How long error, load and stillness must all hold before the joint is held, ms.
    ///
    /// 500 ms is 25 ticks at 50 Hz: past every ordinary transient — a footfall, a direction
    /// reversal, a landing — and a third of a second short of the point where a servo pinned
    /// at 1.4 A has meaningfully warmed.
    pub stall_ms: u64,

    /// Case temperature at which a joint is softened, °C.
    ///
    /// 60 by default: the XL330's own Temperature Limit register is 70 °C, and this robot
    /// pins Shutdown(63) with the overheating bit in it — so 70 is where the servo turns its
    /// own torque off and needs a power cycle. Lower it to *test* the guard on a bench:
    /// setting it under the robot's current case temperature is the only way to watch it fire
    /// without waiting for something to get hot.
    pub overtemp_c: f64,

    /// The position P gain a hot joint runs at.
    ///
    /// Half the running gain by default: low enough to take most of the standing current out
    /// of the joint, high enough that it still holds the robot up.
    pub overtemp_gain: u16,
}

impl Default for PolicyParams {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::Walk,
            walk: None,
            stand: None,
            sitstand: None,
            posture: None,
            ground_pick: None,
            kick_left: None,
            kick_right: None,
            roulade: None,
            action_scale: None,
            standing_action_scale: 1.0,
            // The prototype's `--standing-kp-ratio`.
            standing_gain_ratio: 0.8,
            gain: 200,
            // Both zero: exactly what `set_gain` wrote unconditionally before they were
            // exposed, so a config naming neither behaves as it did before this existed.
            gain_i: 0,
            gain_d: 0,
            head_lowpass: None,
            legs_lowpass: None,
            ground_pick_period: None,
            ground_pick_action_scale: None,
            ground_pick_gain_ratio: 1.0,
            kick_duration: 0.5,
            roulade_duration: 1.0,
            roulade_action_scale: 1.0,
            roulade_gain_ratio: 1.0,
            voltage_adapt: false,
            nominal_voltage: 7.4,
        }
    }
}

impl Default for SafetyParams {
    fn default() -> Self {
        Self {
            fall_gravity_z: -0.5,
            fall_debounce_ms: 200,
            deadman_ms: 500,
            gain_limp: 50,
            // Upstream: a fall preempts nothing. See the field docs for when to turn it on.
            fall_stops_driving: false,
            // Off by default: the "empty battery" verdict comes from the servo bus's
            // `present_input_voltage`, whose 6.6 V threshold is calibrated for the prototype's
            // 2S lithium pack (6.6–8.2 V). This robot's servos are fed 5 V, so the bus reads
            // ~5.0 V and is always judged "empty" — which made robotd power itself off a few
            // seconds after boot (amber then dead, like a short). 5 V is the correct servo
            // rail, not an empty battery, so no automatically-triggered poweroff here.
            battery_empty_shutdown: false,
            low_battery_sit: false,
            low_battery_voltage: 6.2,
            limp_fall: true,
            limp_fall_tilt_z: -0.90,
            limp_fall_predict_z: -0.5,
            limp_fall_lookahead_ms: 300,
            limp_fall_debounce_ms: 60,
            limp_fall_still_rate: 1.0,
            limp_fall_still_ms: 200,
            limp_fall_max_ms: 1500,
            limp_fall_pose_ms: 600,
            limp_fall_pose_gain: 160,
            // The per-joint guards, on. See the field docs above for what they answer.
            joint_limit_clamp: true,
            stall_protection: true,
            overtemp_protection: true,
            // 12°: an order of magnitude under the 69° standing error of the accident, and
            // above the lag a low-kp servo shows while it is legitimately tracking.
            stall_error_deg: 12.0,
            // 900 mA: 61% of the XL330-M288's 5 V stall current (datasheet: 0.52 N·m at
            // 1.47 A) and half of the servo's own Current Limit register (1750 mA).
            stall_current_ma: 900.0,
            // 800 of the 885 PWM Limit allows.
            stall_pwm: 800.0,
            stall_ms: 500,
            // 60 °C: the servo's own overheat shutdown is at 70 (Temperature Limit register
            // default), and this leaves 10 °C for the core to run hotter than the case it
            // reports. Idle case temperature is about 32 °C.
            overtemp_c: 60.0,
            // Half the running gain.
            overtemp_gain: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bus {
    /// Serial port the servos and the IMU board share. The Radxa Zero 3W wires them to
    /// `/dev/ttyS2`.
    pub port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Control {
    /// Control loop rate. 50 Hz is inherited from the prototype, where it was chosen on a
    /// Pi Zero 2W — re-derive it on the Radxa rather than trusting it.
    pub hz: u32,
    /// Per-tick EMA on the velocity command: `cmd += α × (target − cmd)`. The prototype's
    /// `--cmd-alpha` — what turns a stick snap into a ramp the gait can follow. `1.0` is
    /// pass-through.
    pub cmd_alpha: f64,
    /// Same, for head targets and the body pose.
    pub head_alpha: f64,
}

/// Thresholds that decide `healthy` — and therefore whether an update is kept.
///
/// **Not** the thresholds for everything `robot.health` reports. That answer also describes the
/// battery, the motor temperatures and the loop counters, and none of those may reach a verdict
/// `[health]` invited exactly that mistake: it reads like "how the robot is doing", when what it
/// configures is the one question auto-rollback turns on.
///
/// Everything here is a property of the *software*. A future `[thermal]` section for a motor
/// temperature that should throttle the robot would be a different thing, and belongs under a
/// different name.
///
/// The section was called `[health]`. Renamed outright rather than aliased: a board carrying
/// the old name gets a parse error naming the section, which is a better outcome than a robot
/// quietly running on default thresholds nobody chose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct UpdateGate {
    /// Below this achieved rate the robot reports unhealthy, which is what makes the
    /// updater's auto-rollback mean something. A loop running at 60% of target is alive,
    /// answers every request, and is badly broken.
    pub min_achieved_hz: f64,
    /// How many periods may pass with no tick before the loop counts as **wedged**.
    ///
    /// This detects a dead loop, not a slow one — `min_achieved_hz` owns degradation. Keep
    /// the two apart: set this near the period and it fires on ordinary scheduler jitter,
    /// which on a loaded board would report a perfectly good release unhealthy and roll it
    /// back. A loop that has not ticked in half a second is genuinely gone; one that
    /// ticked 80 ms late is just late.
    pub stall_periods: u32,
    /// Consecutive bus read failures tolerated before reporting unhealthy. One dropped
    /// transaction is ordinary; a run of them means the bus is gone.
    pub max_consecutive_errors: u32,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            port: "/dev/ttyS2".into(),
        }
    }
}

impl Default for Control {
    fn default() -> Self {
        Self {
            hz: 50,
            cmd_alpha: 0.2,
            head_alpha: 0.2,
        }
    }
}

impl Default for UpdateGate {
    fn default() -> Self {
        Self {
            // 90% of the default rate. Generous enough not to trip on a slow tick, tight
            // enough that a loop losing every tenth cycle is not called healthy.
            min_achieved_hz: 45.0,
            // 500 ms at the default rate. Deliberately far from the period: three periods
            // is 60 ms, which ordinary scheduler jitter exceeds on a busy machine, and a
            // health check that trips on jitter rolls back good releases.
            stall_periods: 25,
            max_consecutive_errors: 10,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParamsError {
    #[error("{path}: safety.low_battery_voltage must be finite and between 6.0 and 8.2 V, got {got}")]
    BatteryVoltage { path: String, got: f64 },
    #[error("{path}: invalid IMU configuration: {message}")]
    Imu {path:String,message:String},
    #[error("reading {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{path}: control.hz must be between 1 and 1000, got {got}")]
    Rate { path: String, got: u32 },
    #[error(
        "{path}: media.bitrate must be between {min} and {max} bits per second, got {got} — \
         the unit is bits, so 2 Mb/s is 2000000"
    )]
    Bitrate {
        path: String,
        got: u32,
        min: u32,
        max: u32,
    },
}

/// The band `media.bitrate` is accepted in, bits per second.
///
/// The floor is not taste: it is where a typo lands. `bitrate = 2000` is somebody who meant
/// kilobits, and 2 kb/s is a stream that never produces a picture — far better refused at the
/// editor than debugged off a board. The ceiling is what the link and the VPU are for; above it
/// the encoder is being asked for something no robot's wifi will carry.
pub const BITRATE_MIN: u32 = 100_000;
pub const BITRATE_MAX: u32 = 20_000_000;

impl Params {
    /// Load from `path`. A missing file at the *default* location is not an error — an
    /// unprovisioned board should still come up on defaults rather than refuse to start,
    /// and a daemon that will not start is much harder to diagnose remotely than one
    /// running on known defaults. A file explicitly named on the command line must exist.
    pub fn load(path: &Path, explicit: bool) -> Result<Self, ParamsError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => {
                tracing::warn!(path = %path.display(), "no params file; using defaults");
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(ParamsError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };

        // Strict first. A file this build fully understands parses exactly as it always has —
        // one pass, serde's own spans, no second guess — and that is the overwhelmingly common
        // case. The lenient path below only ever runs on a file that has already failed.
        let params = match toml::from_str::<Params>(&text) {
            Ok(params) => params,
            // Not an unknown-key problem: a syntax error, or a value of the wrong type. The
            // strict error is what gets reported, because it is the one carrying a line and a
            // column.
            Err(source) => {
                let Some((reparsed, ignored)) = without_unknown_keys(&text) else {
                    return Err(ParamsError::Parse {
                        path: path.display().to_string(),
                        source,
                    });
                };
                if ignored.iter().any(|key|key.starts_with("imu.")) {
                    return Err(ParamsError::Parse{path:path.display().to_string(),source});
                }
                tracing::warn!(
                    path = %path.display(),
                    ignored = %ignored.join(", "),
                    "this build has no such keys; they are ignored and their values do nothing"
                );
                // Pruned, and what is left still does not parse — a real error sharing a file
                // with an inert one. This reports the real one, which costs the position: serde
                // is now looking at a table rather than at the text. The alternative was worse.
                // Returning the strict error here would name the key this release just declared
                // harmless as the reason the daemon will not start, and send whoever reads it to
                // delete a section that was never the problem.
                reparsed.map_err(|source| ParamsError::Parse {
                    path: path.display().to_string(),
                    source,
                })?
            }
        };
        params.validate(path)?;
        Ok(params)
    }

    /// Reject values that would produce a loop that cannot work, at startup rather than as
    /// a division by zero three seconds later.
    fn validate(&self, path: &Path) -> Result<(), ParamsError> {
        let v = self.safety.low_battery_voltage;
        if !v.is_finite() || !(6.0..=8.2).contains(&v) {
            return Err(ParamsError::BatteryVoltage { path: path.display().to_string(), got: v });
        }
        let m=&self.imu;
        let valid=match m.model.as_str() {
            "lsm6dsv16x"=>[0x6a,0x6b].contains(&m.i2c_addr)&&["sflp","comp"].contains(&m.fusion.as_str()),
            "bno085"=>[0x4a,0x4b].contains(&m.i2c_addr)&&m.fusion=="game_rv",_=>false,
        };
        if !["bus","i2c"].contains(&m.source.as_str()) || !valid || !m.i2c_dev.starts_with("/dev/i2c-")
            || !m.g_filter_k.is_finite() || !(0.0..=1.0).contains(&m.g_filter_k) || !m.accel_trust_g.is_finite()
            || m.accel_trust_g<=0.0 || m.lever_arm.iter().any(|v|!v.is_finite()) {
            return Err(ParamsError::Imu{path:path.display().to_string(),message:"model/device/address/fusion or filter values do not match".into()});
        }
        if self.control.hz == 0 || self.control.hz > 1000 {
            return Err(ParamsError::Rate {
                path: path.display().to_string(),
                got: self.control.hz,
            });
        }
        // Checked here rather than in `mediad`, so `robotctl configure` refuses to write it:
        // the daemon that would choke on this one is not the daemon whose gate the editor runs.
        if let Some(bitrate) = self.media.bitrate
            && !(BITRATE_MIN..=BITRATE_MAX).contains(&bitrate)
        {
            return Err(ParamsError::Bitrate {
                path: path.display().to_string(),
                got: bitrate,
                min: BITRATE_MIN,
                max: BITRATE_MAX,
            });
        }
        Ok(())
    }

    pub fn period(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(1.0 / self.control.hz as f64)
    }
}

/// Re-parse a file that `deny_unknown_fields` rejected, dropping the keys this build has no
/// place for, and say which they were. `None` when nothing was dropped — the parse failed for
/// some other reason and the caller should report that instead.
///
/// **Why unknown keys are no longer fatal.** They were, and the reasoning was sound as far as it
/// went: a silently ignored `min_acheived_hz` leaves an operator believing they moved a threshold
/// they did not. What that argument missed is the other way a key becomes unknown — the build
/// changed underneath a file nobody typed into. A robot running a branch had `[chorale]` in its
/// `robotd.toml`; updating it to a `main` without that feature produced a `robotd` that would not
/// start, four consecutive rollbacks, and a bench session spent on a robot that was fine. The
/// section was inert. Refusing to run over it is a far larger penalty than the mistake it guards
/// against, and it lands on exactly the transitions — a downgrade, a branch, a release that
/// dropped a feature — where the operator did nothing wrong at all.
///
/// So the value is kept and the enforcement moved: every dropped key is named at `warn`, and
/// `robotctl configure` writes only keys the registry knows. What is gone is a robot that will
/// not walk because of a line in a config file that does nothing.
///
/// **The registry is the authority on what a key is**, not serde. It has to be: serde's answer
/// arrives as prose inside an error, and this needs the question asked per key. That is safe
/// because it is not a second copy of the schema —
/// [`registry::tests::the_registry_covers_every_key_exactly`] pins it to [`Params`] in both
/// directions, so a key the registry does not know is a key `Params` does not have.
///
/// `deny_unknown_fields` stays on the structs. It is what makes that test possible, and it is
/// the backstop here: if the registry ever did drift, the pruned table would still be rejected
/// rather than quietly deserialised into something else.
#[allow(clippy::type_complexity)]
fn without_unknown_keys(text: &str) -> Option<(Result<Params, toml::de::Error>, Vec<String>)> {
    let mut table: toml::Table = text.parse().ok()?;
    let mut ignored: Vec<String> = Vec::new();

    table.retain(|section, value| {
        let Some(fields) = value.as_table_mut() else {
            // A bare value at the top level — `hz = 50` written outside any section, which is
            // the shape a hand-edit takes when someone forgets the header. No registry key can
            // name it, and reporting it by its bare name is what tells them why.
            ignored.push(section.to_string());
            return false;
        };
        if !registry::has_section(section) {
            // Reported as the section rather than as each of its keys: `[chorale]` is one
            // decision someone made, not four mistakes.
            ignored.push(format!("[{section}]"));
            return false;
        }
        fields.retain(|key, _| {
            if registry::entry_for(&format!("{section}.{key}")).is_some() {
                true
            } else {
                ignored.push(format!("{section}.{key}"));
                false
            }
        });
        true
    });

    if ignored.is_empty() {
        // Nothing here was an unknown key, so the strict parse failed for a reason this cannot
        // help with. `None` says exactly that, and keeps the caller's two cases apart.
        return None;
    }
    ignored.sort();
    Some((toml::Value::Table(table).try_into::<Params>(), ignored))
}

#[cfg(test)]
mod tests {

    #[test]
    fn playback_defaults_have_no_removed_sensor_options() {
        let params = Params::default();
        let value = serde_json::to_value(&params).expect("serialize defaults");
        assert!(value.get("theremin").is_none());
        let audio = value["audio"].as_object().expect("audio settings");
        for key in ["pet_detect", "pet_model", "pet_enter_threshold", "pet_exit_threshold"] {
            assert!(!audio.contains_key(key), "removed option: {key}");
        }
        assert!(params.audio.enabled);
        assert!(params.audio.greet);
    }
    use super::*;

    fn write(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("robotd.toml");
        std::fs::write(&path, body).unwrap();
        path
    }

    /// An unprovisioned board must still come up. A daemon that refuses to start because a
    /// config file is absent is far harder to diagnose on a robot than one running on
    /// documented defaults.
    #[test]
    fn a_missing_default_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = Params::load(&dir.path().join("absent.toml"), false).unwrap();
        assert_eq!(p.control.hz, 50);
    }

    /// But a file named explicitly on the command line must exist — silently ignoring
    /// `--params /path/typo.toml` would run the robot on settings nobody chose.
    #[test]
    fn an_explicitly_named_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Params::load(&dir.path().join("absent.toml"), true).is_err());
    }

    /// Partial files are the normal case — a board overrides the port and nothing else.
    #[test]
    fn absent_sections_take_their_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[bus]\nport = \"/dev/ttyUSB0\"\n");
        let p = Params::load(&path, true).unwrap();
        assert_eq!(p.bus.port, "/dev/ttyUSB0");
        assert_eq!(p.control.hz, 50);
        assert_eq!(p.update_gate.stall_periods, 25);
    }

    /// [`QUALITY_LABELS`] is what the registry offers and what the file may contain, and
    /// [`Quality::ALL`] is what the daemon can do — a rung in one and not the other is either a
    /// choice the editor writes and `mediad` cannot read, or a mode nobody can select.
    #[test]
    fn every_quality_label_round_trips() {
        assert_eq!(QUALITY_LABELS.len(), Quality::ALL.len());
        for (label, quality) in QUALITY_LABELS.iter().zip(Quality::ALL) {
            assert_eq!(*label, quality.label());
            let parsed: Params =
                toml::from_str(&format!("[media]\nquality = \"{label}\"\n")).expect("parses");
            assert_eq!(parsed.media.quality, quality);
        }
    }

    /// The labels are `webrtcsink`'s own property nicknames — the strings that get set on the
    /// element. `gcc`, not `googcc`: a nickname this file spelled its own way would be a config
    /// key that parses, validates, saves, and then silently leaves the element on its default.
    #[test]
    fn every_congestion_label_round_trips() {
        assert_eq!(CONGESTION_LABELS.len(), CongestionControl::ALL.len());
        for (label, mode) in CONGESTION_LABELS.iter().zip(CongestionControl::ALL) {
            assert_eq!(*label, mode.nick());
            let parsed: Params =
                toml::from_str(&format!("[media]\ncongestion_control = \"{label}\"\n"))
                    .expect("parses");
            assert_eq!(parsed.media.congestion_control, mode);
        }
        // `gcc` is webrtcsink's own default, so a robot with no key set must land there — naming
        // it must not change what every robot has been running.
        assert_eq!(
            MediaParams::default().congestion_control,
            CongestionControl::Gcc
        );
    }

    /// The starting bitrate follows the picture unless somebody says otherwise — the whole
    /// reason `bitrate` is optional rather than a number to keep in step by hand.
    #[test]
    fn an_unset_bitrate_follows_the_quality() {
        let mut media = MediaParams::default();
        for quality in Quality::ALL {
            media.quality = quality;
            assert_eq!(media.bitrate_resolved(), quality.default_bitrate());
        }
        media.bitrate = Some(3_000_000);
        assert_eq!(media.bitrate_resolved(), 3_000_000);
    }

    /// A bitrate in the wrong unit is the mistake this band exists to catch: `2000` is somebody
    /// who meant kilobits, and it would produce a stream with no picture in it.
    #[test]
    fn a_bitrate_in_kilobits_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[media]\nbitrate = 2000\n");
        assert!(Params::load(&path, true).is_err());
        let path = write(dir.path(), "[media]\nbitrate = 2000000\n");
        assert_eq!(
            Params::load(&path, true).unwrap().media.bitrate_resolved(),
            2_000_000
        );
    }

    /// Today's shipped behaviour, pinned: a robot with no `[media]` section streams its camera
    /// at exactly what `mediad`'s flags used to default to. This section changed where those
    /// numbers live and must not have changed the numbers.
    #[test]
    fn the_defaults_are_what_mediad_streamed_before_the_section_existed() {
        let media = Params::default().media;
        assert!(media.camera, "mediad.service carried --camera");
        assert_eq!(media.quality.size(), (1280, 720));
        assert_eq!(media.quality.fps(), 30);
        assert_eq!(media.bitrate_resolved(), 2_000_000);
    }

    #[test]
    fn low_battery_threshold_is_configurable_and_validated() {
        let mut p: Params = toml::from_str("[safety]\nlow_battery_sit=true\nlow_battery_voltage=6.2").unwrap();
        assert!(p.safety.low_battery_sit);
        assert_eq!(p.safety.low_battery_voltage, 6.2);
        assert_eq!(Params::default().safety.low_battery_voltage, 6.2);
        assert!(p.validate(Path::new("test")).is_ok());
        for v in [0.0, -1.0, f64::NAN, f64::INFINITY, 8.3] {
            p.safety.low_battery_voltage = v;
            assert!(matches!(p.validate(Path::new("test")), Err(ParamsError::BatteryVoltage { .. })));
        }
    }

    /// The shipped example must agree with the built-in defaults, or the file documents a
    /// robot that does not exist — and an operator reading it would draw wrong conclusions
    /// about what their board is actually doing.
    #[test]
    fn the_shipped_example_matches_the_defaults() {
        let shipped = include_str!("../../test-fixtures/robotd-defaults.toml");
        let from_file: Params = toml::from_str(shipped).expect("deploy/robotd.toml must parse");
        let built_in = Params::default();

        assert_eq!(from_file.bus.port, built_in.bus.port);
        assert_eq!(from_file.control.hz, built_in.control.hz);
        assert_eq!(from_file.control.cmd_alpha, built_in.control.cmd_alpha);
        assert_eq!(from_file.control.head_alpha, built_in.control.head_alpha);
        assert_eq!(from_file.policy.resolved(), built_in.policy.resolved());
        assert_eq!(from_file.safety.limp_fall, built_in.safety.limp_fall);
        assert_eq!(
            from_file.safety.battery_empty_shutdown,
            built_in.safety.battery_empty_shutdown
        );
        assert_eq!(
            from_file.update_gate.min_achieved_hz,
            built_in.update_gate.min_achieved_hz
        );
        assert_eq!(
            from_file.update_gate.stall_periods,
            built_in.update_gate.stall_periods
        );
        assert_eq!(
            from_file.update_gate.max_consecutive_errors,
            built_in.update_gate.max_consecutive_errors
        );
        assert_eq!(from_file.media.camera, built_in.media.camera);
        assert_eq!(from_file.media.quality, built_in.media.quality);
        assert_eq!(
            from_file.media.bitrate_resolved(),
            built_in.media.bitrate_resolved()
        );
        assert_eq!(
            from_file.media.congestion_control,
            built_in.media.congestion_control
        );
    }

    /// The resolved walk-mode defaults are the prototype's **current alpha configuration**
    /// — the values `microduck_runtime` ships as built-in defaults, which its installer
    /// deliberately passes no flags to override. Changing any of these silently changes how
    /// the robot moves relative to the thing this daemon replaces.
    #[test]
    fn walk_mode_resolves_to_the_prototype_alpha_config() {
        let p = Params::default().policy.resolved();
        assert_eq!(p.mode, Mode::Walk);
        assert_eq!(p.action_scale, 0.9);
        assert_eq!(p.standing_action_scale, 1.0);
        assert_eq!(p.standing_gain_ratio, 0.8, "--standing-kp-ratio");
        assert_eq!(p.gain, 200);
        assert_eq!(
            p.head_lowpass,
            Some(0.5),
            "trained with the filter ON at 0.5"
        );
        assert_eq!(
            p.legs_lowpass,
            Some(0.7),
            "trained with the filter ON at 0.7"
        );
        assert_eq!(p.ground_pick_period, 4.0);
        assert_eq!(p.ground_pick_action_scale, 1.0);
        assert_eq!(p.ground_pick_gain_ratio, 1.0);
        assert_eq!(p.kick_duration, 0.5);
        assert_eq!(p.roulade_duration, 1.0, "one roll, the measured time");
        assert_eq!(p.roulade_action_scale, 1.0);
        assert_eq!(p.roulade_gain_ratio, 1.0);
        assert!(!p.voltage_adapt, "off by default in the prototype");
        assert_eq!(p.nominal_voltage, 7.4);

        let name = |p: &Option<std::path::PathBuf>| {
            p.as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        };
        assert!(p.walk.ends_with("policies/alpha_walking.onnx"));
        assert_eq!(name(&p.stand).as_deref(), Some("alpha_stand.onnx"));
        assert_eq!(name(&p.sitstand).as_deref(), Some("alpha_sitstand.onnx"));
        assert_eq!(
            name(&p.ground_pick).as_deref(),
            Some("alpha_ground_pick.onnx")
        );
        assert_eq!(name(&p.kick_left).as_deref(), Some("ball_kick_left.onnx"));
        assert_eq!(name(&p.kick_right).as_deref(), Some("ball_kick_right.onnx"));
        assert_eq!(name(&p.roulade).as_deref(), Some("roulade.onnx"));
    }

    /// Command smoothing matches the prototype's `--cmd-alpha` / `--head-alpha`.
    #[test]
    fn command_smoothing_defaults_match_the_prototype() {
        let c = Control::default();
        assert_eq!(c.cmd_alpha, 0.2);
        assert_eq!(c.head_alpha, 0.2);
    }

    /// One line — `mode = "roller"` — must reproduce the prototype's whole roller preset,
    /// which its installer rebased on the alpha defaults: the roller policy and its tuning
    /// (kp 200, scale 0.8, the crouch on the ground-pick trigger at 3 s / 0.8), and
    /// everything else exactly as walking mode has it — sit/stand, kicks, roulade, the
    /// trained low-pass. Only the standing network stays out (the prototype loads it and
    /// then skips every standing transition in roller mode, so it never runs).
    #[test]
    fn roller_mode_resolves_to_the_prototype_roller_preset() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[policy]\nmode = \"roller\"\n");
        let p = Params::load(&path, true).unwrap().policy.resolved();

        assert_eq!(p.mode, Mode::Roller);
        assert!(p.walk.ends_with("policies/roller.onnx"));
        assert_eq!(
            p.stand, None,
            "the prototype never runs standing in roller mode"
        );
        assert!(
            p.sitstand
                .as_ref()
                .unwrap()
                .ends_with("alpha_sitstand.onnx"),
            "the rebased roller line keeps the sit"
        );
        assert!(
            p.kick_left
                .as_ref()
                .unwrap()
                .ends_with("ball_kick_left.onnx")
        );
        assert!(
            p.kick_right
                .as_ref()
                .unwrap()
                .ends_with("ball_kick_right.onnx")
        );
        assert!(p.roulade.as_ref().unwrap().ends_with("roulade.onnx"));
        assert!(
            p.ground_pick
                .as_ref()
                .unwrap()
                .ends_with("roller_crouch.onnx")
        );
        assert_eq!(p.action_scale, 0.8);
        assert_eq!(p.ground_pick_period, 3.0);
        assert_eq!(p.ground_pick_action_scale, 0.8);
        assert_eq!(
            p.head_lowpass,
            Some(0.5),
            "the rebased roller line keeps the trained filters"
        );
        assert_eq!(p.legs_lowpass, Some(0.7));
        assert_eq!(p.gain, 200);
    }

    /// `"none"` disables an optional slot outright — the prototype's `--sitstand-policy None`
    /// convention — and `1.0` turns a low-pass into a pass-through, which is how its preset
    /// spells "off".
    #[test]
    fn none_and_unity_are_the_off_switches() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "[policy]\nsitstand = \"None\"\nhead_lowpass = 1.0\n",
        );
        let p = Params::load(&path, true).unwrap().policy.resolved();
        assert_eq!(p.sitstand, None);
        assert_eq!(
            p.head_lowpass, None,
            "alpha 1.0 is a pass-through, so store it as off"
        );
        assert_eq!(
            p.legs_lowpass,
            Some(0.7),
            "the other filter keeps its default"
        );
    }

    /// A typo in a key is named and ignored, and the setting it was aimed at keeps its default.
    ///
    /// It used to be fatal, on the argument that silently ignoring `min_acheived_hz` leaves the
    /// operator believing they moved a threshold they did not. The value of that is real and is
    /// why the key is named at `warn`; what it does not justify is a robot that will not start.
    /// See [`without_unknown_keys`].
    #[test]
    fn a_typo_is_ignored_and_leaves_the_real_key_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[update_gate]\nmin_acheived_hz = 10.0\n");
        let p = Params::load(&path, true).expect("a typo must not stop the robot starting");
        assert_eq!(
            p.update_gate.min_achieved_hz,
            UpdateGate::default().min_achieved_hz,
            "the misspelt key must not have moved the real one"
        );

        let (_, ignored) = without_unknown_keys("[update_gate]\nmin_acheived_hz = 10.0\n")
            .expect("an unknown key is what this file has");
        assert_eq!(ignored, ["update_gate.min_acheived_hz"]);
    }

    /// The renamed section, reported as the section rather than as each key under it.
    ///
    /// `install.sh` never overwrites `robotd.toml`, so a board carrying `[health]` keeps it
    /// across every update. That used to mean a `robotd` that would not start; it now means a
    /// line in the journal naming the section and a robot that walks.
    #[test]
    fn the_old_health_section_name_is_ignored_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[health]\nmin_achieved_hz = 40.0\n");
        assert!(Params::load(&path, true).is_ok());

        let (_, ignored) = without_unknown_keys("[health]\nmin_achieved_hz = 40.0\n").unwrap();
        assert_eq!(ignored, ["[health]"], "one decision, not one line per key");
    }

    /// The incident this came from. A robot running the `duck-chorale` branch had `[chorale]`
    /// in its `robotd.toml`; `main` has no such feature, so the update to it produced a `robotd`
    /// that exited on every start, a health gate that timed out, and four rollbacks in a row —
    /// over a section that does nothing.
    #[test]
    fn a_section_from_another_branch_does_not_stop_the_robot() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "[control]\nhz = 50\n\n[chorale]\naccept = true\n",
        );
        let p = Params::load(&path, true).expect("an inert section must not be fatal");
        assert_eq!(p.control.hz, 50, "the rest of the file must still be read");
    }

    /// The other half, so leniency cannot become "accept anything". A value of the wrong type is
    /// still an error, with its position, exactly as before.
    #[test]
    fn a_bad_value_is_still_rejected_with_its_position() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[control]\nhz = \"fast\"\n");
        let err = Params::load(&path, true)
            .expect_err("a string where a number belongs is not something to warn about")
            .to_string();
        assert!(err.contains("line"), "{err}");
    }

    /// A real error sharing a file with an inert one. The error must be about `hz`, and must not
    /// be about `[chorale]` — naming the section this release just declared harmless as the
    /// reason the daemon will not start is how someone spends an afternoon deleting the wrong
    /// thing.
    #[test]
    fn a_bad_value_beside_an_unknown_section_names_the_bad_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "[chorale]\naccept = true\n\n[control]\nhz = \"fast\"\n",
        );
        let err = Params::load(&path, true)
            .expect_err("the file still does not parse")
            .to_string();
        assert!(err.contains("hz"), "{err}");
        assert!(!err.contains("chorale"), "{err}");
    }

    /// And a file that is not TOML at all fails as it always did, rather than being pruned into
    /// something that parses.
    #[test]
    fn a_syntax_error_is_still_a_syntax_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[control\nhz = 50\n");
        assert!(Params::load(&path, true).is_err());
    }

    /// A file this build understands completely takes the strict path and reports nothing.
    #[test]
    fn a_clean_file_has_nothing_to_report() {
        assert!(without_unknown_keys("[control]\nhz = 50\n").is_none());
    }

    /// Zero would divide by zero when computing the period; absurdly high would spin.
    #[test]
    fn an_impossible_rate_is_rejected_at_startup() {
        let dir = tempfile::tempdir().unwrap();
        for hz in ["0", "5000"] {
            let path = write(dir.path(), &format!("[control]\nhz = {hz}\n"));
            assert!(Params::load(&path, true).is_err(), "hz = {hz} was accepted");
        }
    }
}

#[cfg(test)]
mod commanded_posture_slot_tests {
    use super::*;
    #[test]
    fn posture_does_not_replace_recovery() {
        let p: PolicyParams=toml::from_str("sitstand='getup.onnx'\nposture='posture.onnx'\n").unwrap();
        let r=p.resolved();assert_eq!(r.sitstand,Some("getup.onnx".into()));assert_eq!(r.posture,Some("posture.onnx".into()));
        assert!(PolicyParams::default().resolved().posture.is_none());
    }
}

#[cfg(test)] mod imu_contract_tests {
    use super::*;
    #[test] fn legacy_and_two_chips() {
        let old:Params=toml::from_str("[imu]\nsource='i2c'\ni2c_dev='/dev/i2c-2'\ni2c_addr=107\nfusion='sflp'").unwrap();assert_eq!(old.imu.model,"lsm6dsv16x");assert!(old.validate(Path::new("test")).is_ok());
        let mut b=old;b.imu.model="bno085".into();b.imu.i2c_addr=75;assert!(b.validate(Path::new("test")).is_err());b.imu.fusion="game_rv".into();assert!(b.validate(Path::new("test")).is_ok());b.imu.model="unknown".into();assert!(b.validate(Path::new("test")).is_err());
    }
    #[test] fn misspelled_imu_key_is_not_silently_pruned() {
        let path=std::env::temp_dir().join(format!("easyduck-imu-config-{}.toml",std::process::id()));
        std::fs::write(&path,"[imu]\nmodle='bno085'\n").unwrap();
        let result=Params::load(&path,true);std::fs::remove_file(&path).unwrap();assert!(result.is_err());
    }

}
