# Build, installation, and service integration

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Connects repository source to application binaries, a staged filesystem, and managed Linux services. Build, install, verify, rollback, and startup are separate operations. SDK/kernel/system-image construction remains outside this application repository.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [Dockerfile](Dockerfile) | Pinned Rust toolchain and cross-build dependencies |
| [build.sh](build.sh) | Fresh source staging and ARM64 workspace build |
| [install.py](install.py) | Model verification, installation mapping, backups, and installed-file verification |
| [rollback.py](rollback.py) | Manifest-driven file/link/permission restoration |
| [start.sh](start.sh) | Preview or explicit startup of normal services |
| [dependencies.lock.json](dependencies.lock.json) | Reference toolchain and system/license requirements |
| [network_routes.py](network_routes.py) | Shared reachable-address and navigation helpers |
| [navigation-client.js](navigation-client.js) | Browser-side page navigation/address handoff |
| [systemd](systemd) | Service units and drop-ins |
| [setup](setup) | Wi-Fi/Bluetooth setup and service handoff |
| [kernel](kernel) | Board kernel inputs and image-side gamepad checks |

## Call and data flow

1. build.sh uses an existing build container to produce four binaries without starting board services.
2. install.py combines binaries, verified models, public profiles, assets, service inputs, and license records in a chosen root; existing configuration is preserved unless explicitly replaced.
3. start.sh creates required system identities and starts normal services only when executed; rollback restores manifest files without starting them. SDK integration consumes kernel/gamepad inputs separately.

## Subdirectories

| Directory | Description |
| --- | --- |
| [alsa](alsa/README.md) | Board audio state |
| [kernel](kernel/README.md) | Board kernel integration inputs |
| [modules-load.d](modules-load.d/README.md) | Gamepad boot-load rule |
| [network-access](network-access/README.md) | Local naming and IP announcements |
| [setup](setup/README.md) | Wi-Fi/Bluetooth setup service |
| [systemd](systemd/README.md) | Linux service units and overrides |
| [sysusers.d](sysusers.d/README.md) | System identities for application access |

## Boundaries and change notes

- An application install cannot add missing kernel drivers, ONNX Runtime, or all system packages. A rootfs check is not a flashing/boot/physical acceptance test.
- Live installation, service restart, configuration replacement, and rollback require robot support and their own authorization. Never package a used robot's calibration or credentials as defaults.

## Further reading

- [Detailed guide](../docs/en/deployment.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
