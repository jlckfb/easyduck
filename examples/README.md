# Developer examples

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Collects deliberately separate development skeletons. Examples explain extension contracts without being installed as active drivers or added to the normal Cargo workspace.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [imu](imu) | Paired Rust/Python skeletons and their bilingual explanation |
| [../runtime/duck-control/src/imu_driver.rs](../runtime/duck-control/src/imu_driver.rs) | Actual Rust extension contract |
| [../calibration/imu](../calibration/imu) | Actual Python reader implementation |

## Call and data flow

1. Developers use the example as a starting point, implement device behavior, then register real drivers in the application modules.
2. Applications use the actual driver directories, not this example folder.

## Subdirectories

| Directory | Description |
| --- | --- |
| [imu](imu/README.md) | IMU driver skeletons |

## Boundaries and change notes

- An incomplete example is not proof of hardware support and should fail visibly rather than fabricate sensor values.
- Keep private configurations and generated artifacts out of examples. Update both languages when changing an extension contract.

## Further reading

- [Detailed guide](../docs/en/imu-drivers.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
