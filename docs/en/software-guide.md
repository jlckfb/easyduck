# Software guide

**English** | [简体中文](../zh-CN/software-guide.md)

## Runtime structure

```text
Web controller (9527) / padd -> robotd -> HD1910 servos (/dev/ttyS3)
                                     -> LSM6DSV16X / BNO085 IMU (default /dev/i2c-2)
                                     -> ONNX policies and audio
Calibration (8766) -> exclusive servo serial and IMU access
```

`robotd` controls the robot, `padd` reads gamepads, `robotctl` provides queries/terminal operations, and `sounds` generates audio. Web and calibration services use Python. Run commands from the repository root unless stated otherwise.

## System requirements

- TaishanPi 1M, Linux ARM64, HD1910 servos, and LSM6DSV16X or BNO085 configured for actual hardware. See [IMU drivers](imu-drivers.md).
- Complete board software needs UART3 GPIO116 half-duplex support, Python 3.11+, ONNX Runtime 1.23+, libudev, ALSA `aplay` / `amixer`, and systemd. Kernel inputs are under `deploy/kernel/`.
- Bluetooth gamepads require BlueZ, `bluetooth.service`, and input-device permissions; `padd` reads input events.
- Tested USB/2.4 GHz receivers need matching `xpad` and boot loading; application installation cannot supply the kernel driver. See [USB gamepads](usb-gamepad.md).
- Builds need Linux, Docker, Rust, an ARM64 cross-compiler, and target libraries. See `deploy/dependencies.lock.json` and `runtime/Cargo.lock`.

## Build

On a Linux x86-64 host, from the repository root:

```sh
docker build -f deploy/Dockerfile -t easyduck-build:1.98.1 .
docker run -d --name easyduck-build --network none easyduck-build:1.98.1
bash deploy/build.sh
```

Image creation downloads system packages, fixed Rust, and Cargo dependencies; it needs no existing build container, sysroot, or personal Cargo cache. Application builds use `--locked --offline` without network. Reuse stopped containers with `docker start easyduck-build`; after dependency updates, rebuild and choose a new container name. See [deployment](deployment.md).

Programs `robotd`, `padd`, `robotctl`, and `sounds` appear in `build/bin/`; logs go to `build/logs/`.

## Unit tests and file verification

```sh
(cd runtime && cargo test --locked --workspace)
```

Run with Rust and native libudev development libraries on Linux, not by executing cross-built ARM64 programs. In-source tests use simulated inputs and require no robot. Separate local test scripts are not prerequisites for using the software. Verify bundled models:

```sh
(cd models && sha256sum -c SHA256SUMS)
```

## Configuration

Start with `config/examples/`. Policies are disabled; HOME 2048 illustrates format, not physical calibration. `config/board/` supplies TaishanPi interfaces. Both profiles are uncalibrated templates, not measured settings from a robot.

| File | Purpose |
| --- | --- |
| robotd.toml | Serial, frequency, IMU, models, audio |
| visual_calibration.json | Joint HOME, direction, limits; IMU installation/bias |
| zero_pos.json | Servo baseline HOME, direction, limits |
| scs_joint_gains.json | Per-joint P overrides |
| voice.env | Audio-generation seed |

Runtime settings live in `/etc/robot/` and are read at startup; restart after changes. The servo driver prefers `visual_calibration.json`. If absent, it uses built-in HOME/direction and limits from `zero_pos.json`; HOME/direction in that file must match the built-in baseline or startup fails. Generate robot-specific visual calibration through the workbench. See [configuration](configuration.md).

## Staged installation and rollback

Install into a test directory without starting services:

```sh
python3 deploy/install.py --root build/stage --profile examples
# Replace BACKUP_DIR with the backup directory printed by the installer.
python3 deploy/install.py --root build/stage --verify BACKUP_DIR/manifest.json
python3 deploy/rollback.py --root build/stage --backup BACKUP_DIR
```

Installation includes `/opt/robot/licenses/THIRD_PARTY_NOTICES.md`, with hashes and rollback. Existing settings are preserved unless `--replace-config` explicitly requests replacement and backup. Rollback handles manifest files only and does not start services.

## Board installation and startup

Stop control/calibration services, support the robot, confirm torque release, and prepare matching settings before installation:

```sh
python3 deploy/install.py --root / --profile examples --install-on-board
bash deploy/start.sh
# Review the preview before starting services.
bash deploy/start.sh --execute
```

Complete physical calibration before motion with either template. `--profile board` selects hardware references, not finished calibration. Upgrades preserve existing board calibration by default.

- Controller: `http://BOARD_IP:9527`.
- Calibration: `http://BOARD_IP:8766`; starting `xgoduck-calibration.service` stops normal control to avoid competing serial access.
- Service queries: `systemctl status robotd padd xgoduck-web`.
- Logs: `journalctl -u robotd -f`.

Before startup, `sounds` generates `/var/lib/robot/sounds/` using `voice.env`; matching existing banks are reused.

## Board rollback

Stop control/calibration services and select the installation backup:

```sh
python3 deploy/rollback.py --root / --backup BACKUP_DIR --restore-on-board
systemctl daemon-reload
```

Rollback does not start services or energize servos. Before reuse, check that configuration, policies, and programs match.

## Daily use

- Start `robotd` and `xgoduck-web` before browser control. Torque/action buttons affect hardware.
- Pair a Bluetooth gamepad through the system, or connect a supported USB receiver, then run `padd`. The daemon coordinates browser/gamepad ownership.
- Read-only inspection uses `robotctl robot mode --json` or `robotctl monitor`. See [command-line operations](robotctl.md).
- For manual calibration service management, run `systemctl start xgoduck-calibration.service` to stop normal control; afterward stop calibration and restore normal services using the startup steps.
- A stopped control service does not prove servo torque is off. Confirm release before handling the robot.

## Physical checks

Check serial feedback, joint directions/limits, IMU direction, supported torque-on, standing, and each action in order. Compilation, mock tests, and model-structure checks cannot establish correct physical motion.

## Related documentation

- [Build, installation, and rollback](deployment.md)
- [Servo and IMU workbench](calibration.md)
- [Sound generation](sounds.md)
- [Policy interfaces](models.md)
- [Third-party licenses and provenance](../../THIRD_PARTY_NOTICES.md)

### Gamepad walking speeds

In walking mode, a fully forward stick requests 0.25 m/s for the first 100 ms, then 0.15 m/s. Full reverse requests −0.30 m/s for 100 ms, then −0.25 m/s. Partial travel scales the sustained speed; recenter sends zero forward/backward speed immediately.

Full travel is detected at 95%; returning to 90% or less rearms the initial boost to avoid repeated triggers from endpoint noise. Direction changes, head/body mode switches, and disconnection clear timing. Full left requests at most 0.55 m/s; full right remains 0.30 m/s. These are policy commands, not measured physical walking speeds.

`padd --max-linear` sets sustained forward speed; `--max-linear-backward` reverse; `--max-lateral-left` left; `--max-lateral` right. Initial boost factors are 5/3 and 1.2 for 100 ms. Browser control and wheeled mode have their own settings.

### Gamepad sit and stand

In the `xgoduck` control profile, enable the policy first, then press D-pad **down** to request sitting; press again to request standing. `policy.posture` selects this network, while `policy.sitstand` retains fall recovery.

While sitting, the robot holds the seated pose and sticks do not drive walking. Request standing, let the robot settle, then move the drive stick to return to walking. Start still disables/enables as before. Pickup, roulade, and posture actions are mutually exclusive; the original recovery process handles falls.

`models/xgoduck_posture_14999_tspi.onnx` was trained with the official task source and includes board direction adaptation. Validate behavior against this robot's calibration and hardware.
