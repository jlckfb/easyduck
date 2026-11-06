# duck-ipc-proto — shared message contract

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Defines the data exchanged between robot services and clients: JSON-RPC envelopes, method names, request/result structures, error codes, telemetry, and shared joint names. It supplies types and encoding/decoding rules, not service processes or hardware transport loops.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Lightweight serialization dependencies and optional test-support feature |
| [src/lib.rs](src/lib.rs) | Protocol constants, socket names, Call/Request/Response/Error, and method payloads |
| [../robotd/src/main.rs](../robotd/src/main.rs) | Daemon-side decoding and method handling consumer |
| [../padd/src/main.rs](../padd/src/main.rs) | Gamepad request producer |
| [../robotctl/src/main.rs](../robotctl/src/main.rs) | CLI request producer and result consumer |

## Call and data flow

1. padd and robotctl build typed requests; robotd decodes them and returns typed results or errors over caller-managed Unix sockets.
2. duck-control reuses joint names so telemetry arrays and model/driver naming cannot drift independently.
3. The Python gateway implements its compatible wire requests separately; changing the Rust types alone does not update Python callers.

### Direct workspace relationships

- Local crate dependencies: None inside this workspace.
- Direct consumers: [duck-control](../duck-control/README.md), [padd](../padd/README.md), [robotctl](../robotctl/README.md), [robotd](../robotd/README.md), [robotd-params](../robotd-params/README.md)

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Preserve compatibility across producers and consumers, including field meanings and array order, not only JSON syntax.
- Retained configd/updaterd methods and types do not provide those external services. Do not confuse their default socket constants with robotd's socket.

## Further reading

- [Detailed guide](../../docs/en/runtime.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
