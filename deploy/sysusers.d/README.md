# System identities for application access

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

systemd-sysusers declarations for the robot socket group and dedicated gamepad user. They establish application access identities, not the web-login account or a default password.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [robot.conf](robot.conf) | Creates the robot group for local IPC access |
| [padd.conf](padd.conf) | Creates the dedicated padd user/group |
| [../start.sh](../start.sh) | Invokes systemd-sysusers before normal startup |
| [../systemd/padd.service](../systemd/padd.service) | Grants input/robot supplementary groups to padd |

## Call and data flow

1. install.py copies declarations to usr/lib/sysusers.d; start.sh explicitly invokes systemd-sysusers.
2. Service unit identity/group settings then control access to Linux input devices and the robotd socket.

## Boundaries and change notes

- robot.conf declares a group, not a robot login user. Keep account/password provisioning separate.
- Do not solve permission errors by broadly opening hardware/socket access. Check actual target identities and merged service permissions.

## Further reading

- [Detailed guide](../../docs/en/deployment.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
