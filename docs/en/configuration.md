# Configuration

**English** | [简体中文](../zh-CN/configuration.md)

- `config/examples/` provides format examples for a new robot. Policies are disabled, and HOME 2048 is an example, not usable physical calibration. Set HOME, direction, limits, and IMU installation before use.
- `config/board/` provides TaishanPi hardware reference settings. Calibration is also an uncalibrated template and policies are disabled.

## Daemon parameters

`robotd.toml` configures serial communication, loop frequency, policies, IMU, and audio. Reference settings use `/dev/ttyS3` at 50 Hz and `/dev/i2c-2`, address `0x6B`, with SFLP.

Base gains are P=6 and D=20. The reference P in `scs_joint_gains.json` is 6, with no per-joint overrides. Check gains against the robot's actual mechanics and load.

## Calibration files

- `visual_calibration.json`: HOME, direction, limits, and IMU installation/bias; takes precedence in the driver.
- `zero_pos.json`: calibration workbench baseline. When the visual file is absent, the daemon reads limits here and requires HOME/direction to match its built-in baseline. Editing this file alone cannot change daemon HOME values.
- `calibration_speed_restore.json`: servo speed settings to restore after calibration.

Both profiles use placeholder HOME 2048, direction −1, and limits 0–4095. All joints are listed in `limits_pending`; `saved_at` is empty, with no recorded whole-body pose. Both IMUs have identity rotation, zero bias, and `mount_valid=false`: installation has not been calibrated. Confirm each joint's HOME, direction, and range, then record the body pose and calibrate the IMU.

Old single-robot snapshots were archived outside installation inputs. Measured calibration belongs in each robot's `/etc/robot/`, with separate backups identified by robot. Upgrades preserve existing settings by default; do not overwrite calibrated machines with templates.

The daemon reads `/etc/robot/` at startup. The installer preserves existing settings unless `--replace-config` explicitly requests replacement and backup.

## Audio

`voice.env` sets the generation seed; the default is `3239431280`. `sounds` generates the bank before service startup; generated WAV files do not belong in source control.

## Profile selection and installation

Select a complete profile with `--profile examples` or `--profile board`. Both set `policy.enabled=false`; disabling policies does not mean every control command is incapable of energizing servos. Check calibration and robot state before startup or motion.

Keep Wi-Fi passwords, SSH keys, and tokens out of reference profiles. Private settings can use ignored `config/private/`. See [build and deployment](deployment.md).
