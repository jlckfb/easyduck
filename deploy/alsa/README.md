# Board audio state

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

ALSA mixer state used by application installation for the target audio hardware. It complements the runtime audio device configuration and speaker-selection startup command; it is not an audio synthesis module.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [asound.state](asound.state) | Recorded target mixer/control state |
| [../systemd/robotd.service.d/50-audio-speaker.conf](../systemd/robotd.service.d/50-audio-speaker.conf) | Speaker selection and sound-bank preparation |
| [../../runtime/sounds/src/main.rs](../../runtime/sounds/src/main.rs) | Sound CLI that generates/plays audio |

## Call and data flow

1. install.py places asound.state under var/lib/alsa; system ALSA restoration and the service drop-in prepare output routing.
2. robotd/sounds use the device and bank specified by runtime settings; this file contains no robot motion logic.

## Boundaries and change notes

- Mixer controls depend on the board/card; do not assume this snapshot fits another device.
- File installation or sound rendering does not prove audible output. Playback changes are separate from physical motion and still need appropriate authorization.

## Further reading

- [Detailed guide](../../docs/en/sounds.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
