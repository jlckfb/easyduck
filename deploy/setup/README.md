# Wi-Fi/Bluetooth setup service

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Provides authenticated system connection setup and coordinates handoff between normal control and calibration. It uses NetworkManager and BlueZ rather than a second robot-control implementation; motion remains robotd's responsibility.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [first_setup.py](first_setup.py) | HTTP/session authentication, setup APIs, and reconnect entry |
| [connections.py](connections.py) | Validated nmcli/bluetoothctl backend and connection records |
| [navigation.py](navigation.py) | Serialized service handoff, relax request, and exit checks |
| [install.sh](install.sh) | Standalone setup installation, backups, and service enablement |
| [static](static) | Login/setup/navigation UI |
| [../systemd/easyduck-gamepad-reconnect.timer](../systemd/easyduck-gamepad-reconnect.timer) | Periodic retry schedule for registered controllers |

## Call and data flow

1. The browser logs in using the selected existing system account; first_setup routes connection requests to connections.py.
2. connections.py invokes system networking/Bluetooth tools and verifies actual state; the reconnect timer revisits trusted registered devices.
3. Navigator uses systemctl, robotd IPC, and local calibration/control APIs to hand off hardware ownership after confirmation and state checks.

## Subdirectories

| Directory | Description |
| --- | --- |
| [static](static/README.md) | Connection setup browser assets |

## Boundaries and change notes

- Standalone setup installation does not create the system account, SSH configuration, USB networking, or gamepad kernel drivers.
- Setup login does not authenticate the separate control/calibration APIs. Switching services can release support torque; confirm physical support and do not treat accepted requests as guaranteed hardware completion.

## Further reading

- [Detailed guide](../../docs/en/connection-setup.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
