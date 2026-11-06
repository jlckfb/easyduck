# robotd — robot control daemon

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

The normal-operation hardware owner. It receives local control requests, manages input ownership and action state, runs policies against feedback, and publishes status. The installed XGO profile targets a 50 Hz update loop; browser and gamepad clients do not write the servo bus themselves.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/main.rs](src/main.rs) | Startup, IPC handlers, scheduling, hardware/policy orchestration |
| [src/control.rs](src/control.rs) | Controller state and policy-action integration |
| [src/xgo.rs](src/xgo.rs) | XGO action sequencing and transitions |
| [src/control_owner.rs](src/control_owner.rs) | Browser/gamepad ownership and neutral-input readiness |
| [src/intents.rs](src/intents.rs) | Shared command snapshots and pending requests |
| [src/low_battery.rs](src/low_battery.rs) | Low-battery preemption, sitting/release state, and latch |
| [src/sound.rs](src/sound.rs) | Audio playback, live vocalization, and mouth coordination |
| [src/chorale.rs](src/chorale.rs) | Chorale runtime coordination |
| [src/soc.rs](src/soc.rs) | Linux thermal-zone sampling |
| [src/params.rs](src/params.rs) | Re-export of the shared robotd-params schema |
| [src/main_power_tests.rs](src/main_power_tests.rs) | Mocked main-loop power transition tests |

## Call and data flow

1. `padd`, the web gateway, and `robotctl` send requests through `/run/robotd.sock`; the daemon validates and records intent rather than handing clients the bus.
2. The loop combines accepted intent with `duck-control` sensor snapshots and policy observations, then applies action-state/protection decisions before servo output.
3. Telemetry feeds browser/terminal monitors; audio uses `sounds` synthesis and daemon playback. Calibration runs as a separate, conflicting service.

### Direct workspace relationships

- Local crate dependencies: [duck-control](../duck-control/README.md), [duck-ipc-proto](../duck-ipc-proto/README.md), [kinematics](../kinematics/README.md), [robotd-params](../robotd-params/README.md), [sounds](../sounds/README.md)
- Direct consumers: None inside this workspace.

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Do not equate process startup, policy loading, torque-on, and walking. Templates disable policies; physical motion remains an explicit operation.
- The generic controller and XGO path differ. Retained protocol/body/kick features do not imply that the installed XGO loop executes them. Process restart is not proof of released servo torque.

## Further reading

- [Detailed guide](../../docs/en/runtime.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
