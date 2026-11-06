# Rust runtime

**English** | [简体中文](../zh-CN/runtime.md)

The complete Cargo workspace is under `runtime/`. Build through `deploy/build.sh`; `Cargo.lock` pins dependencies.

| Type | Component | Responsibility |
| --- | --- | --- |
| Program | robotd | 50 Hz main control, action transitions, status publication |
| Program | padd | Send gamepad intent to the daemon |
| Program | robotctl | Status queries, terminal operations, diagnostics |
| Program | sounds | Generate the robot sound bank |
| Shared crate | duck-control | HD1910 communication, I2C IMU, inference, control protection |
| Shared crate | duck-ipc-proto | Client/daemon messages and joint order |
| Shared crate | robotd-params | Configuration parsing and parameter definitions |
| Shared crate | kinematics | Pose geometry |

The four shared crates compile into programs; they are not separate services. `runtime/test-fixtures/` contains test inputs, not board deployment settings.

`runtime/duck-control/tests/fixtures/scs_calibration_test_fixture.json` is a fixed automated-test sample and is not deployed. `config/board/zero_pos.json` is the uncalibrated installation baseline; each robot uses its own `/etc/robot/visual_calibration.json`.

See the [software guide](software-guide.md) for build, test, and installation steps.
