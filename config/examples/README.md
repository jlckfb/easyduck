# Generic configuration template

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

An installable, uncalibrated profile selected with --profile examples. Policies are disabled and physical joint/IMU calibration is pending. The generic profile does not explicitly opt into board low-battery sitting.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [robotd.toml](robotd.toml) | Hardware, policy, audio, and protection settings |
| [visual_calibration.json](visual_calibration.json) | Joint/IMU placeholders and pending calibration flags |
| [zero_pos.json](zero_pos.json) | Baseline format and fallback limits |
| [scs_joint_gains.json](scs_joint_gains.json) | Gain references and overrides |
| [calibration_speed_restore.json](calibration_speed_restore.json) | Servo speed restoration values |
| [voice.env](voice.env) | Deterministic audio seed |

## Call and data flow

1. install.py selects this complete profile for /etc/robot; existing target files are preserved by default.
2. robotd and calibration read their relevant settings; the user must generate robot-specific calibration before motion.

## Boundaries and change notes

- Placeholder HOME/direction/ranges and identity IMU mounting are not measured calibration. Do not copy another robot's values here.
- Changes here affect installation inputs, not a running daemon immediately. Preserve the sibling profile and keep private credentials out.

## Further reading

- [Detailed guide](../../docs/en/configuration.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
