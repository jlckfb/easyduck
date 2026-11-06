# Servo and IMU calibration service

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Runs the independent hardware workbench. It manages servo discovery/ID assignment, hardware center, HOME, direction/ranges, motion jobs, IMU installation/bias, drafts, and saved calibration. The Python backend is the hardware owner while calibration is active.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [server.py](server.py) | HTTP/API entry, application state, validation, and exit coordination |
| [hardware.py](hardware.py) | Physical serial bus and MockBus implementations |
| [motion.py](motion.py) | Bounded motion jobs and stop handling |
| [servo_setup.py](servo_setup.py) | ID discovery/write/verification and journaling |
| [calibration_math.py](calibration_math.py) | Joint/reference-pose and calibration mathematics |
| [imu_settings.py](imu_settings.py) | Shared robotd.toml IMU selection validation and persistence |
| [imu](imu) | Chip drivers, transport, and shared IMU processing |
| [static](static) | Workbench UI, 3D meshes, and vendored browser libraries |
| [dependencies.lock.json](dependencies.lock.json) | Vendored dependency versions/hashes |

## Call and data flow

1. The workbench sends JSON commands to server.py; backend operations use hardware/motion/servo_setup and shared application state.
2. The IMU package consumes the same hardware settings as robotd and applies model-specific mounting/bias calibration.
3. Authenticated setup navigation stops normal services before calibration; exit validates drafts/motion/torque, closes hardware, and allows normal services to reopen saved settings.

## Subdirectories

| Directory | Description |
| --- | --- |
| [imu](imu/README.md) | Calibration IMU readers |
| [static](static/README.md) | Calibration browser frontend |

## Boundaries and change notes

- Hardware-center writes are permanent servo operations; HOME capture and saving software drafts are different operations. Confirm the selected physical joint before writes.
- Use --mock for hardware-free previews. The port 8766 API does not provide independent account authentication; service inactivity or successful navigation does not prove physical torque release.

## Further reading

- [Detailed guide](../docs/en/calibration.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
