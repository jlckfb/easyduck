# Browser controller and gateway

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Serves the operator controller and translates browser requests into robotd IPC. Python owns HTTP, control-session/heartbeat handling, status collection, and request validation; JavaScript owns page interaction. The browser does not generate joint trajectories or run ONNX.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [server.py](server.py) | RPC client, Gateway, snapshots, WebSocket heartbeat, HTTP endpoints |
| [assets/index.html](assets/index.html) | Controller layout and navigation elements |
| [assets/app.js](assets/app.js) | Joysticks, buttons, keyboard, input cleanup, and ownership UI |
| [assets/style.css](assets/style.css) | Controller appearance and responsive layout |
| [tests/test_gateway.py](tests/test_gateway.py) | Mocked daemon/gateway regression checks |
| [tests/controls.test.cjs](tests/controls.test.cjs) | Browser interaction checks with mocked networking |
| [requirements.lock](requirements.lock) | Component Python dependency declaration |

## Call and data flow

1. The browser sends accepted interaction requests to server.py; Gateway manages the current page connection and forwards control through `/run/robotd.sock`.
2. robotd status and subscription data are assembled into snapshots for the UI. Shared navigation/address helpers come from deploy.
3. Page handoff goes through the setup service; calibration, rather than the web gateway, is the alternate hardware owner.

## Subdirectories

| Directory | Description |
| --- | --- |
| [assets](assets/README.md) | Controller browser assets |

## Boundaries and change notes

- Control ownership is not account authentication. This service listens on port 9527 by default; its APIs require a trusted network.
- The web controller exposes walking/head/posture/pickup controls with current model/state limits; roulade/jumping are rejected. UI labels are currently Chinese, including when reading English documentation.

## Further reading

- [Detailed guide](../docs/en/web-control.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
