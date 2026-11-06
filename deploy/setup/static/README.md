# Connection setup browser assets

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Static login, connection settings, and service-navigation pages. They display results from the authenticated setup service rather than invoking Linux tools directly.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [login.html](login.html) | Login form and server-provided session flow |
| [index.html](index.html) | Wi-Fi/Bluetooth setup layout |
| [setup.js](setup.js) | Network/gamepad actions and incremental status rendering |
| [setup.css](setup.css) | Theme and responsive layout |
| [navigation.html](navigation.html) | Handoff confirmation/progress page |
| [navigation.js](navigation.js) | Confirmation, CSRF data, state checks, and destination navigation |

## Call and data flow

1. first_setup.py serves templates and authenticated APIs; connections.py performs system networking/Bluetooth changes.
2. Navigation requests reach Navigator, which coordinates local services and hardware handoff before returning a destination.

## Boundaries and change notes

- UI confirmation must correspond to actual backend state; never report a guessed connection result as success.
- Changing Wi-Fi or service ownership can interrupt access/control. Keep live form values and do not remove authentication/confirmation checks for appearance changes.

## Further reading

- [Detailed guide](../../../docs/en/connection-setup.md)
- [Documentation index](../../../docs/en/README.md)
- [Repository maintenance rules](../../../AGENTS.md)
