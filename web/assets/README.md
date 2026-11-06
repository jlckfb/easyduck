# Controller browser assets

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

The static controller interface served by web/server.py. It handles user input, ownership/status display, and connection cleanup; it does not independently decide servo targets or run motion models.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [index.html](index.html) | Page controls, status panels, and navigation |
| [app.js](app.js) | Joystick/keyboard/buttons, WebSocket heartbeat, release/recenter behavior |
| [style.css](style.css) | Responsive layout and interaction appearance |
| [../server.py](../server.py) | HTTP assets and gateway API owner |

## Call and data flow

1. User events become web API requests and connection heartbeats; the Python Gateway forwards permitted intent to robotd.
2. Daemon-derived status updates the controls; focus loss, stale input, and navigation clear/release browser input according to the client flow.

## Boundaries and change notes

- Do not add a second direct servo path here. Keep frontend command ranges and backend validation consistent.
- Documentation translation does not localize these UI strings. Offline mocked interaction checks do not prove hardware behavior.

## Further reading

- [Detailed guide](../../docs/en/web-control.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
