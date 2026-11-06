# Linux service units and overrides

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Declarative application process management for systemd: startup commands, dependencies, permissions, restart settings, and normal-control/calibration exclusion. Units orchestrate processes rather than implementing control algorithms.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [robotd.service](robotd.service) | Main daemon and robot socket permissions |
| [padd.service](padd.service) | Gamepad client user/groups and restart settings |
| [xgoduck-web.service](xgoduck-web.service) | Python controller service |
| [xgoduck-calibration.service](xgoduck-calibration.service) | Exclusive calibration process |
| [easyduck-setup.service](easyduck-setup.service) | Authenticated connection setup process |
| [easyduck-gamepad-reconnect.timer](easyduck-gamepad-reconnect.timer) | Registered-controller retry timing |
| [easyduck-gamepad-reconnect.service](easyduck-gamepad-reconnect.service) | One-shot reconnect command |
| [easyduck-network-access.service](easyduck-network-access.service) | Local address announcement process |
| [robotd.service.d/40-xgoduck-reference.conf](robotd.service.d/40-xgoduck-reference.conf) | Select XGO control profile |
| [robotd.service.d/50-audio-speaker.conf](robotd.service.d/50-audio-speaker.conf) | Speaker selection and sound-bank preparation |
| [padd.service.d/60-control-owner.conf](padd.service.d/60-control-owner.conf) | Managed-control command arguments |
| [xgoduck-web.service.d/60-control-owner.conf](xgoduck-web.service.d/60-control-owner.conf) | Web control service override |

## Call and data flow

1. install.py places units/drop-ins under the target systemd directory; start.sh reloads systemd and starts the normal services when explicitly executed.
2. Setup Navigator coordinates handoff; systemd Conflicts prevents calibration and normal services from running together. Restart policies recover process exits, not every hardware fault.

## Boundaries and change notes

- Installation, service startup, and boot enablement are different operations. Inspect merged units/drop-ins rather than a base file alone.
- Stopping/restarting services can affect support torque; never infer physical torque-off from an inactive unit. Changes require board-specific verification before deployment.

## Further reading

- [Detailed guide](../../docs/en/deployment.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
