# Calibration IMU readers

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Python sensor drivers and common pose processing used by the calibration service. Chip code reports newly received measurements; shared code validates settings, tracks capabilities/freshness, and applies model-specific installation/bias. It mirrors the Rust data contract without calling the Rust driver.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [__init__.py](__init__.py) | Model registration, driver opening, common processing, and calibration selection |
| [transport.py](transport.py) | Linux I2C descriptor/transaction handling |
| [lsm6dsv16x_reader.py](lsm6dsv16x_reader.py) | LSM reports and supported fusion modes |
| [bno085.py](bno085.py) | Independent BNO SH-2/SHTP parsing |
| [../imu_settings.py](../imu_settings.py) | robotd.toml configuration selection and persistence |

## Call and data flow

1. calibration/server.py opens the configured driver and passes reports through the shared processor to workbench state and calibration sampling.
2. The saved profile carries mounting/bias per model; robotd later reads matching calibration through its Rust implementation.

## Boundaries and change notes

- Missing data must stay missing, not become fabricated zeroes or refreshed timestamps. Keep quaternion order, units, body frame, and timing aligned with Rust.
- Never run this reader concurrently with robotd on the same device. Preserve existing GPL notices and verify new chip support on hardware.

## Further reading

- [Detailed guide](../../docs/en/imu-drivers.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
