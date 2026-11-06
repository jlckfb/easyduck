# Gamepad boot-load rule

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Supplies the systemd modules-load configuration requesting xpad at boot. It names a module; it does not contain or compile the kernel driver.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [easyduck-gamepad.conf](easyduck-gamepad.conf) | Requests xpad for compatible Xbox/XInput receivers |
| [../kernel/easyduck-gamepad.config](../kernel/easyduck-gamepad.config) | SDK kernel configuration input |
| [../kernel/check_gamepad.py](../kernel/check_gamepad.py) | Checks rule/module/version/alias in staged images |

## Call and data flow

1. The application installer puts the rule in etc/modules-load.d.
2. At boot the system loader uses a module already packaged by the SDK; Linux input then feeds padd.

## Boundaries and change notes

- A rule without a matching xpad module cannot add support. Module release/ABI and depmod indexes must match the image.
- Successful module loading alone does not prove receiver pairing, button mapping, or robot motion.

## Further reading

- [Detailed guide](../../docs/en/usb-gamepad.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
