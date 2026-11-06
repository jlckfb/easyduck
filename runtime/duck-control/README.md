# duck-control — hardware and inference library

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

A library compiled into robotd, not a separate service. It defines the robot I/O boundary, servo/model coordinate conversions, IMU readers, policy observations/inference, and reusable target protection. Linux hardware implementations and fake I/O share interfaces.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/lib.rs](src/lib.rs) | Public module/export entry |
| [src/io.rs](src/io.rs) | RobotIo, Sensors, JointTargets, and FakeIo abstractions |
| [src/scs.rs](src/scs.rs) | ScsIo serial packets, joint calibration, feedback, and torque operations |
| [src/model.rs](src/model.rs) | Joint IDs/order, model reference, directions, and ranges |
| [src/obs.rs](src/obs.rs) | 61-value observation and 14-action ordering, excluding mouth |
| [src/policy.rs](src/policy.rs) | Dynamic ONNX Runtime loading, sessions, and inference |
| [src/imu_driver.rs](src/imu_driver.rs) | Shared driver contract, timed reports, and body-frame processing |
| [src/imu_i2c.rs](src/imu_i2c.rs) | LSM6DSV16X reader |
| [src/imu_bno085.rs](src/imu_bno085.rs) | BNO085 SH-2/SHTP implementation |
| [src/imu_transport.rs](src/imu_transport.rs) | Bounded Linux I2C transport |
| [src/protection.rs](src/protection.rs) | Reusable command/target protection |
| [src/safety.rs](src/safety.rs) | Safety helpers |
| [src/fall.rs](src/fall.rs) | Fall-state helpers |
| [src/io_trace.rs](src/io_trace.rs) | Optional feedback/inference/wire tracing |

## Call and data flow

1. robotd constructs the selected I/O and policy objects and owns their lifetime; this crate does not run an IPC server.
2. Servo feedback and IMU snapshots become an observation; inference produces model actions that are converted into calibrated servo targets.
3. Joint names are shared from duck-ipc-proto. Calibration uses a separate Python implementation with the same IMU conventions, rather than importing this Rust crate.

### Direct workspace relationships

- Local crate dependencies: [duck-ipc-proto](../duck-ipc-proto/README.md)
- Direct consumers: [robotd](../robotd/README.md)

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- ONNX Runtime is loaded dynamically when needed; successful compilation does not prove a usable target runtime or model.
- Preserve joint order, units, calibration signs, freshness, and the model contract. FakeIo and trace replay do not simulate physical contact or validate motion. Helper presence does not prove every controller path uses it.

## Further reading

- [Detailed guide](../../docs/en/models.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
