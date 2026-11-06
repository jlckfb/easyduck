# Local naming and IP announcements

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Provides offline address announcements and the application Avahi naming configuration. It observes network/robot state and defers speech while policies are active; it does not provision Wi-Fi or control servos.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [network_access.py](network_access.py) | Address stability, status subscription, offline speech generation/playback, and status file |
| [avahi-daemon.conf](avahi-daemon.conf) | Application mDNS naming configuration |
| [../systemd/easyduck-network-access.service](../systemd/easyduck-network-access.service) | Startup/restart and filesystem permissions |
| [../systemd/avahi-daemon.service.d/50-easyduck.conf](../systemd/avahi-daemon.service.d/50-easyduck.conf) | Avahi override selecting the application configuration |

## Call and data flow

1. System networking supplies addresses and Avahi supplies the actual registered local name.
2. The Python service subscribes to robotd state, uses eSpeak NG/ALSA for speech, and writes status under /run/easyduck-network.

## Boundaries and change notes

- mDNS support depends on the network/client. Spoken IP is a fallback, not a fixed address guaranteed across hotspots.
- Do not change route, DNS, hotspot credentials, or robot calibration to fix an announcement. Speech rendering success does not prove audible playback.

## Further reading

- [Detailed guide](../../docs/en/network-access.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
