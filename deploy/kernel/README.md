# Board kernel integration inputs

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Inputs consumed by a matching external board SDK. UART half-duplex support and gamepad kernel support are separate requirements; these files do not build or install a Linux kernel on their own.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [uart3-halfduplex-20261021.patch](uart3-halfduplex-20261021.patch) | UART3 half-duplex patch for its recorded baseline |
| [easyduck-gamepad.config](easyduck-gamepad.config) | Kernel configuration fragment for input/xpad support |
| [check_gamepad.py](check_gamepad.py) | Read-only staged-image config/module/vermagic/alias checks |
| [tests/test_gamepad.py](tests/test_gamepad.py) | Checker regression cases with mocked system commands |
| [../modules-load.d/easyduck-gamepad.conf](../modules-load.d/easyduck-gamepad.conf) | Boot-loading input installed by the application |

## Call and data flow

1. The SDK applies matching kernel inputs and builds/packages modules; the application installer supplies the module-load rule and checker.
2. SDK rootfs/final-image verification invokes check_gamepad.py to inspect packaged files and module indexes, without loading drivers or moving the robot.

## Boundaries and change notes

- Kernel modules must match the packaged kernel release and ABI. A configuration fragment or serial device node alone is insufficient.
- Offline module/alias checks are not boot or controller-input acceptance. Keep the tested receiver scope explicit rather than promising all 2.4 GHz devices.

## Further reading

- [Detailed guide](../../docs/en/usb-gamepad.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
