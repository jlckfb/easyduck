# padd — gamepad input client

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Reads normalized gamepad axes/buttons and translates them into robot intent. Linux input drivers and gilrs handle device representation; padd handles movement modes, button actions, command timing, and IPC. A separate read-only input tap supports diagnostics.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [Cargo.toml](Cargo.toml) | gilrs, IPC, and Linux evdev/libudev-related dependencies |
| [src/main.rs](src/main.rs) | Device events, mapping, driving/head modes, speed shaping, and managed-control messages |
| [src/tap.rs](src/tap.rs) | Linux raw event observation and bounded subscriber queues |
| [../../deploy/systemd/padd.service](../../deploy/systemd/padd.service) | Runtime user, input/socket permissions, and restart policy |
| [../../deploy/systemd/padd.service.d/60-control-owner.conf](../../deploy/systemd/padd.service.d/60-control-owner.conf) | Installed managed-control and command-range arguments |

## Call and data flow

1. USB/2.4 GHz receivers use the matching kernel driver; Bluetooth pads use the system Bluetooth/input stack. Both appear as Linux input devices.
2. gilrs events feed mapping and mode logic in `main.rs`; requests go to robotd, which decides ownership and execution.
3. `robotctl monitor` can subscribe to the tap for diagnosis. The tap does not own or command servos.

### Direct workspace relationships

- Local crate dependencies: [duck-ipc-proto](../duck-ipc-proto/README.md)
- Direct consumers: None inside this workspace.

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Pairing or loading xpad is outside this program. Supported mappings do not establish compatibility with every controller.
- Start/walking, pickup, emergency release, and held shutdown are different actions. Losing ownership must not be described as automatically enabling motion. Current worktree edits are not proof of deployed binaries.

## Further reading

- [Detailed guide](../../docs/en/usb-gamepad.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
