# Rust runtime workspace

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Contains four executable programs and four shared crates. Cargo builds their local dependencies together. The normal control path is client requests into robotd, then hardware/inference operations; shared crates do not start independent services.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Workspace members, default programs, shared Rust edition/version/dependencies |
| [Cargo.lock](Cargo.lock) | Locked dependency resolution |
| [robotd](robotd) | Main control and hardware ownership |
| [padd](padd) | Gamepad intent client |
| [robotctl](robotctl) | Terminal operations and monitoring |
| [sounds](sounds) | Audio generator and library |
| [duck-control](duck-control) | Hardware, sensors, policy I/O/inference |
| [duck-ipc-proto](duck-ipc-proto) | Shared message and joint-name definitions |
| [robotd-params](robotd-params) | Shared startup schema/defaults/validation |
| [kinematics](kinematics) | Geometric pose computations |
| [test-fixtures](test-fixtures) | Test configuration inputs, not robot defaults |

## Call and data flow

1. padd and robotctl submit local requests; the Python web service outside this directory does the same.
2. robotd links control, protocol, parameters, geometry, and audio libraries; the dependency table below is derived from current Cargo manifests.
3. deploy/build.sh cross-builds programs; deploy/install.py installs their binaries with separately maintained models and configuration.

### Workspace dependency map

| Member | Direct local dependencies |
| --- | --- |
| [robotd](robotd/README.md) | `duck-control`, `duck-ipc-proto`, `kinematics`, `robotd-params`, `sounds` |
| [padd](padd/README.md) | `duck-ipc-proto` |
| [robotctl](robotctl/README.md) | `duck-ipc-proto`, `robotd-params` |
| [sounds](sounds/README.md) | None |
| [duck-control](duck-control/README.md) | `duck-ipc-proto` |
| [duck-ipc-proto](duck-ipc-proto/README.md) | None |
| [robotd-params](robotd-params/README.md) | `duck-ipc-proto` |
| [kinematics](kinematics/README.md) | None |

Runtime IPC additionally connects padd/robotctl/web to robotd; socket calls are not Cargo dependencies.

## Subdirectories

| Directory | Description |
| --- | --- |
| [duck-control](duck-control/README.md) | duck-control — hardware and inference library |
| [duck-ipc-proto](duck-ipc-proto/README.md) | duck-ipc-proto — shared message contract |
| [kinematics](kinematics/README.md) | kinematics — pose geometry |
| [padd](padd/README.md) | padd — gamepad input client |
| [robotctl](robotctl/README.md) | robotctl — command-line operations |
| [robotd](robotd/README.md) | robotd — robot control daemon |
| [robotd-params](robotd-params/README.md) | robotd-params — shared configuration schema |
| [sounds](sounds/README.md) | sounds — deterministic audio synthesis |
| [test-fixtures](test-fixtures/README.md) | Default-parameter test inputs |

## Boundaries and change notes

- Source code, deployed binaries, and a generated system image are separate states. Builds do not by themselves deploy or start services.
- Fixtures and FakeIo support software checks, not contact simulation or physical-motion acceptance. Read each member's README before changing an interface.

## Further reading

- [Detailed guide](../docs/en/runtime.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
