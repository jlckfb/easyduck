# Robot sounds

**English** | [简体中文](../zh-CN/sounds.md)

`sounds` generates 48 kHz mono WAV files from a fixed seed for startup, quacks, prompts, and alarms. The seed determines voice character; variants change sounds within a category.

## Sound-bank generation

```sh
sounds ensure-bank --dir /var/lib/robot/sounds --seed 3239431280
```

An existing bank with a matching seed and version is reused. The daemon's audio startup configuration invokes this command; `/etc/robot/voice.env` selects the seed. Generated WAV files do not belong in source control.

## Offline rendering

```sh
sounds render chirp chirp.wav --seed 3239431280 --variant 0
sounds render-all output-sounds --seed 3239431280
```

These commands only write audio files. Use an output directory outside the worktree or an ignored build directory.

## Playback

TaishanPi RK809 audio uses ALSA:

```sh
sounds play chirp --seed 3239431280 --device plughw:0,0
```

`robotd.toml` selects the audio device and bank under `[audio]`. `deploy/alsa/asound.state` supplies card settings; the service selects speaker output before startup.

## Music and live synthesis

- `runtime/sounds/scores/wistful.duckscore`: default text score and format example.
- `runtime/sounds/scores/duck_strut.mid`: bundled MIDI score.
- `runtime/sounds/src/chorale/`: score parsing, voice allocation, beat tracking, rendering.
- The daemon audio module connects playback, chorale, and live audio parameters. Chorale/live controls require the corresponding configuration and input; bank generation does not automatically enable them.

Sound recipes derive from `apirrone/microduck_sounds`; the Rust implementation uses its declared project license. Scores and third-party assets retain their respective terms.
