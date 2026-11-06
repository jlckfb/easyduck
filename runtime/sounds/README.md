# sounds — deterministic audio synthesis

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Provides both a sound-generation CLI and a synthesis library linked by robotd. A seed selects voice character; tags/variants select recipes. Bank generation produces 48 kHz mono WAV files and version/seed metadata, not robot motion.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/main.rs](src/main.rs) | ensure-bank/render/play command entry |
| [src/lib.rs](src/lib.rs) | Bank version, tags, recipes, and bank generation |
| [src/voices.rs](src/voices.rs) | Sound recipes including low-battery alert |
| [src/synth.rs](src/synth.rs) | Synthesis primitives and sample rate |
| [src/personality.rs](src/personality.rs) | Seed-derived voice character |
| [src/rng.rs](src/rng.rs) | Deterministic random generation |
| [src/stream.rs](src/stream.rs) | Streaming sound generation |
| [src/chorale](src/chorale) | Score parsing, timing, voice allocation, and rendering |
| [scores](scores) | Text/MIDI score inputs |

## Call and data flow

1. The audio systemd drop-in runs sounds ensure-bank before daemon startup, using the configured voice seed.
2. robotd's sound module coordinates playback and vocalization/mouth commands; ALSA and device configuration remain system responsibilities.
3. Score/synthesis helpers support chorale paths, but no complete multi-robot discovery/transport service is supplied here.

### Direct workspace relationships

- Local crate dependencies: None inside this workspace.
- Direct consumers: [robotd](../robotd/README.md)

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Keep generated WAV files outside source control. Matching bank seed/version allows reuse; recipe changes may require a bank version change.
- Preserve score/recipe attribution. Successful file rendering does not establish speaker playback, and generating a bank does not enable chorale or motion.

## Further reading

- [Detailed guide](../../docs/en/sounds.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
