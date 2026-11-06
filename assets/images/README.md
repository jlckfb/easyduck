# Hardware photograph slots

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Reserved for real robot, expansion-board, and wiring photographs. No hardware photographs have been added here yet.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [.gitkeep](.gitkeep) | Keeps the empty reserved directory in Git |
| [../../README.md](../../README.md) | English photo-slot consumer |
| [../../README.zh-CN.md](../../README.zh-CN.md) | Chinese photo-slot consumer |

## Call and data flow

1. Add real media here, then add matching relative image references in both READMEs at the chosen showcase position.
2. Use one shared image file and localized descriptions; this directory has no runtime callers.

## Boundaries and change notes

- A reserved directory is not evidence that a photograph exists. Add image tags only after the real file is present.
- Check the photo for visible credentials or private calibration. Keep source code, models, and video originals out of the photo directory.

## Further reading

- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
