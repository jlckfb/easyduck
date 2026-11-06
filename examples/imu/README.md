# IMU driver skeletons

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Minimal Rust and Python outlines for adding the same IMU model to normal control and calibration. Both require device initialization and report decoding; the current skeletons explicitly report unsupported/unimplemented behavior.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [imu-driver-template.rs](imu-driver-template.rs) | Rust Driver implementation outline and capabilities/read/close methods |
| [imu-driver-template.py](imu-driver-template.py) | Python reader outline and capabilities/read/close methods |
| [../../runtime/duck-control/src/imu_driver.rs](../../runtime/duck-control/src/imu_driver.rs) | Production Rust registration/processing contract |
| [../../calibration/imu/__init__.py](../../calibration/imu/__init__.py) | Production Python registration/processing |

## Call and data flow

1. Implement bounded transport, identity checks, report parsing, and cleanup in a real chip driver.
2. Register the model/configuration in both applications and verify shared units, quaternion order, coordinates, timestamps, and calibration behavior.

## Boundaries and change notes

- These files are not compiled/imported by the robot services. Direct execution does not read a sensor.
- Do not hard-code assembly rotation into chip decoding or turn absent measurements into zero. Physical axis checks remain necessary.

## Further reading

- [Detailed guide](../../docs/en/imu-drivers.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
