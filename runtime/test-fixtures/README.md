# Default-parameter test inputs

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Fixed input used by configuration tests. It deliberately describes expected schema/default behavior, not the hardware configuration of the current TaishanPi robot.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [robotd-defaults.toml](robotd-defaults.toml) | Fixed configuration fixture; includes a different serial port and enabled policy |
| [../robotd-params/src/lib.rs](../robotd-params/src/lib.rs) | Schema/default tests consuming the fixture |
| [../robotctl/src/configure.rs](../robotctl/src/configure.rs) | Editor round-trip/parameter tests consuming the fixture |

## Call and data flow

1. Rust tests embed this TOML with include_str! and compare parsing/default/editor behavior.
2. The application installer selects config/examples or config/board instead; test fixtures are not a source of measured robot calibration.

## Boundaries and change notes

- Never copy this file to a live robot merely because a test calls it shipped/default. Its /dev/ttyS2 and enabled policy differ from installation templates.
- Update fixture expectations deliberately when changing schema/defaults, and check both parameter-library and editor consumers.

## Further reading

- [Detailed guide](../../docs/en/configuration.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
