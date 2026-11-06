# robotctl — command-line operations

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

A terminal client for queries, monitoring, control requests, and local TOML editing. It keeps service implementations outside the CLI, distinguishing unavailable services from rejected requests. Its terminal 3D display visualizes received feedback rather than simulating motion.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/main.rs](src/main.rs) | Command parser, service routing, IPC, and exit codes |
| [src/configure.rs](src/configure.rs) | Interactive configuration editor with preserved comments/order |
| [src/monitor.rs](src/monitor.rs) | Live terminal/JSON telemetry, pad tap, and 3D display |
| [src/duck.rs](src/duck.rs) | Embedded duck display geometry support |
| [src/show.rs](src/show.rs) | Status presentation helpers |
| [assets/duck.bin](assets/duck.bin) | Compile-time terminal display mesh, not a policy |

## Call and data flow

1. Robot commands and telemetry use robotd; configure edits a local file using robotd-params validation.
2. Monitoring can subscribe to padd's read-only raw event stream in addition to robotd telemetry.
3. Retained net/system/pairing and update clients route to external configd/updaterd; those servers are not included here.

### Direct workspace relationships

- Local crate dependencies: [duck-ipc-proto](../duck-ipc-proto/README.md), [robotd-params](../robotd-params/README.md)
- Direct consumers: None inside this workspace.

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Read-only queries and motion commands have different consequences; init, relax, skills, and chorale require their own operating conditions.
- Saving settings does not instantly reconfigure the running daemon. Service restarts can affect robot support; the editor needs write permission to the target configuration.

## Further reading

- [Detailed guide](../../docs/en/robotctl.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
