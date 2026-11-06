# Public configuration profiles

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Holds installable format/hardware-reference profiles, not calibration measured from a shipped robot. Profile selection is explicit. Both disable motion policies and mark installation/joint calibration as pending; each physical robot must save its own values.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [examples](examples) | Generic uncalibrated format profile |
| [board](board) | TaishanPi hardware reference, also uncalibrated |
| [board/robotd.toml](board/robotd.toml) | Serial/IMU/policy/audio/safety reference settings |
| [board/visual_calibration.json](board/visual_calibration.json) | Joint/IMU calibration placeholders |
| [board/zero_pos.json](board/zero_pos.json) | Baseline format consumed by calibration/driver fallback |
| [board/scs_joint_gains.json](board/scs_joint_gains.json) | Joint-gain reference and optional overrides |

## Call and data flow

1. deploy/install.py selects examples or board and copies that profile under `/etc/robot/`, preserving existing files by default.
2. robotd reads startup settings; calibration reads the same IMU settings and writes robot-specific visual calibration. robotctl configure edits the daemon TOML using the shared Rust schema.
3. The audio startup configuration reads voice.env for deterministic bank generation.

## Subdirectories

| Directory | Description |
| --- | --- |
| [board](board/README.md) | TaishanPi reference template |
| [examples](examples/README.md) | Generic configuration template |

## Boundaries and change notes

- HOME 2048, direction −1, and encoder limits 0–4095 are placeholders. policy.enabled=false does not mean every non-policy operation is incapable of energizing servos.
- Keep passwords, keys, and robot-specific backups out of public profiles. Changes to defaults must not overwrite deployed calibration. Board and generic safety opt-ins differ; inspect actual TOML.

## Further reading

- [Detailed guide](../docs/en/configuration.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
