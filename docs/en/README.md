# EasyDuck documentation

**English** | [简体中文](../zh-CN/README.md)

Start with the software guide; use deployment for build details and calibration before operating a new robot. Commands may affect hardware: follow each guide's prerequisites. Current web UI labels are Chinese.

| Guide | Contents |
| --- | --- |
| [Software guide](software-guide.md) | Install, configure, start, operate, and roll back |
| [Build and deployment](deployment.md) | Toolchain, staged installation, checks, and rollback |
| [Connection setup](connection-setup.md) | Wi-Fi, PS5 pairing, and service handoff |
| [Network access and spoken IP](network-access.md) | Phone hotspots, local names, and address announcements |
| [USB / 2.4 GHz gamepads](usb-gamepad.md) | Receivers, xpad, and SDK image requirements |
| [Web controller](web-control.md) | Controls, status, offline tests, and navigation |
| [Servo and IMU workbench](calibration.md) | IDs, hardware center, HOME, direction, limits, and IMU |
| [Configuration](configuration.md) | Profiles, precedence, calibration files, and audio seed |
| [Motion policies](models.md) | Policy slots, observation/action contract, and provenance |
| [Low-battery sitting](low-battery.md) | Threshold, sitting/release behavior, and validation limits |
| [Robot sounds](sounds.md) | Sound-bank generation, rendering, and playback |
| [Rust runtime](runtime.md) | Programs, shared crates, and runtime boundaries |
| [Command-line tool](robotctl.md) | Read-only monitoring, parameter editing, and commands |
| [IMU integration](imu-drivers.md) | Data conventions, chip support, calibration, and extensions |

## Directory and module introductions

Each directory README explains responsibilities, actual files, call relationships, and change boundaries.

| Directory | Introduction |
| --- | --- |
| [LICENSES](../../LICENSES/README.md) | Original license texts |
| [assets](../../assets/README.md) | Repository presentation media |
| [assets/demos](../../assets/demos/README.md) | Hardware demonstration GIFs |
| [assets/images](../../assets/images/README.md) | Hardware photograph slots |
| [calibration](../../calibration/README.md) | Servo and IMU calibration service |
| [calibration/imu](../../calibration/imu/README.md) | Calibration IMU readers |
| [calibration/static](../../calibration/static/README.md) | Calibration browser frontend |
| [calibration/static/model](../../calibration/static/model/README.md) | Calibration 3D meshes |
| [config](../../config/README.md) | Public configuration profiles |
| [config/board](../../config/board/README.md) | TaishanPi reference template |
| [config/examples](../../config/examples/README.md) | Generic configuration template |
| [deploy](../../deploy/README.md) | Build, installation, and service integration |
| [deploy/alsa](../../deploy/alsa/README.md) | Board audio state |
| [deploy/kernel](../../deploy/kernel/README.md) | Board kernel integration inputs |
| [deploy/modules-load.d](../../deploy/modules-load.d/README.md) | Gamepad boot-load rule |
| [deploy/network-access](../../deploy/network-access/README.md) | Local naming and IP announcements |
| [deploy/setup](../../deploy/setup/README.md) | Wi-Fi/Bluetooth setup service |
| [deploy/setup/static](../../deploy/setup/static/README.md) | Connection setup browser assets |
| [deploy/systemd](../../deploy/systemd/README.md) | Linux service units and overrides |
| [deploy/sysusers.d](../../deploy/sysusers.d/README.md) | System identities for application access |
| [docs](../README.md) | Bilingual project documentation |
| [examples](../../examples/README.md) | Developer examples |
| [examples/imu](../../examples/imu/README.md) | IMU driver skeletons |
| [models](../../models/README.md) | ONNX policies and provenance |
| [runtime](../../runtime/README.md) | Rust runtime workspace |
| [runtime/duck-control](../../runtime/duck-control/README.md) | duck-control — hardware and inference library |
| [runtime/duck-ipc-proto](../../runtime/duck-ipc-proto/README.md) | duck-ipc-proto — shared message contract |
| [runtime/kinematics](../../runtime/kinematics/README.md) | kinematics — pose geometry |
| [runtime/padd](../../runtime/padd/README.md) | padd — gamepad input client |
| [runtime/robotctl](../../runtime/robotctl/README.md) | robotctl — command-line operations |
| [runtime/robotd](../../runtime/robotd/README.md) | robotd — robot control daemon |
| [runtime/robotd-params](../../runtime/robotd-params/README.md) | robotd-params — shared configuration schema |
| [runtime/sounds](../../runtime/sounds/README.md) | sounds — deterministic audio synthesis |
| [runtime/test-fixtures](../../runtime/test-fixtures/README.md) | Default-parameter test inputs |
| [tools](../../tools/README.md) | Repository maintenance checks |
| [web](../../web/README.md) | Browser controller and gateway |
| [web/assets](../../web/assets/README.md) | Controller browser assets |

## Development examples

[IMU driver skeletons](../../examples/imu/README.md) are incomplete examples and do not run with robot services.

## Project and maintenance

[Project overview and architecture](../../README.md) · [Repository rules](../../AGENTS.md) · [Documentation rules](../AGENTS.md) · [Third-party notices](../../THIRD_PARTY_NOTICES.md)

Original licenses/notices remain authoritative and are not duplicated as translated legal texts. Offline checks do not establish physical-motion acceptance.
