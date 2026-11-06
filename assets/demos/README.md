# Hardware demonstration GIFs

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Contains the five animated recordings shown in both root READMEs. They are stored locally so the gallery does not depend on a regional third-party image host. Files retain their original animation and image proportions.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [sit-stand.gif](sit-stand.gif) | Sit/stand demonstration |
| [head-control.gif](head-control.gif) | Head-control demonstration |
| [pickup.gif](pickup.gif) | Pickup demonstration |
| [walking.gif](walking.gif) | Walking demonstration |
| [standing-balance.gif](standing-balance.gif) | Standing/balance demonstration |

## Call and data flow

1. Both root READMEs embed the same files with translated alt text/captions.
2. Layout dimensions are controlled by README table cells and proportional image dimensions; changing layout does not require re-encoding GIFs.

## Boundaries and change notes

- Retain animation and verify every frame sequence when replacing a file. Do not stretch/crop a recording merely to align captions.
- A demonstration shows recorded behavior, not acceptance of every configuration/action. Runtime code, ONNX files, and original MP4s belong elsewhere.

## Further reading

- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
