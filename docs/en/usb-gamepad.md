# USB / 2.4 GHz gamepads

**English** | [简体中文](../zh-CN/usb-gamepad.md)

A 2.4 GHz gamepad connects through its matching USB receiver, rather than the browser's Bluetooth scan. The tested receiver presents `045e:028e Microsoft X-Box 360 pad` once the controller connects. Linux `xpad` converts it into standard input events, which EasyDuck's existing `padd` gamepad interface reads.

[`deploy/kernel/easyduck-gamepad.config`](../../deploy/kernel/easyduck-gamepad.config) supplies kernel configuration inputs. The SDK builds a matching `xpad.ko` and packages the module and depmod indexes. The application installer installs [`deploy/modules-load.d/easyduck-gamepad.conf`](../../deploy/modules-load.d/easyduck-gamepad.conf) for boot loading. Application installation or a configuration fragment alone cannot replace the kernel module build.

The designated SDK's EasyDuck build runs `deploy/kernel/check_gamepad.py` before packaging and after unpacking the final image. It checks kernel settings, module version, boot loading, and USB alias indexes; incomplete support fails verification. This check does not load a driver, access a board, or send motion commands.

After flashing the complete image, insert the receiver into a USB host port, turn on the gamepad, and select its 2.4 GHz mode. A connected controller should appear as `js0` and an `event` input device, with a connected Xbox pad in `padd` logs. The receiver may present **Receiver Update** while disconnected; that state alone does not indicate a missing driver.

Buttons retain EasyDuck's current functions. Driver support does not change browser/gamepad ownership rules or prove physical-motion acceptance. Image calibration is an uncalibrated template: separately back up the robot's own calibration before flashing.
