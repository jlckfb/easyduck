# Repository presentation media

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Shared media used by English and Chinese READMEs. Photos and animated hardware demonstrations are presentation assets, separate from runtime meshes, policy models, and generated robot audio.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [images](images) | Reserved hardware/expansion-board photographs |
| [demos](demos) | Five local animated demonstrations used by both root READMEs |
| [../README.md](../README.md) | English gallery consumer |
| [../README.zh-CN.md](../README.zh-CN.md) | Chinese gallery consumer |

## Call and data flow

1. Root READMEs reference media with repository-relative paths, allowing GitHub to serve files from the same repository.
2. Both languages share media bytes and provide their own captions; no robot service loads these presentation files.

## Subdirectories

| Directory | Description |
| --- | --- |
| [demos](demos/README.md) | Hardware demonstration GIFs |
| [images](images/README.md) | Hardware photograph slots |

## Boundaries and change notes

- Keep original MP4 source outside this repository unless specifically requested. Preserve actual recordings rather than replacing hardware evidence with generated imagery.
- Keep GIF animation, aspect ratio, and captions accurate. GitHub filters inline CSS, so the current gallery uses table/image attributes and local references.

## Further reading

- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
