# Calibration 3D meshes

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Reference geometry used by the browser workbench. Fifteen STL parts and mapping/provenance metadata support display of the body, head, and limbs; they are not ONNX models or measured servo calibration.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [manifest.json](manifest.json) | Part filenames, scales, and source metadata for the loader |
| [sources.json](sources.json) | Recorded mesh origins and license/hash evidence |
| [trunk_base.stl](trunk_base.stl) | Body reference mesh |
| [head_pitch.stl](head_pitch.stl) | Head assembly mesh example |
| [../app.js](../app.js) | Manifest/STL loading and pose updates |
| [../vendor/STLLoader.js](../vendor/STLLoader.js) | Vendored STL parser |

## Call and data flow

1. app.js fetches the manifest, loads STL parts, and updates displayed transforms from calibration state.
2. Joint signs/HOME and IMU installation affect the displayed pose through the workbench state, not by changing the mesh files.

## Boundaries and change notes

- Preserve part naming, scales, hashes, and attribution when changing geometry. Do not overwrite third-party license records.
- A plausible 3D pose does not prove correct physical axes or safe travel. Verify the actual robot independently.

## Further reading

- [Detailed guide](../../../docs/en/calibration.md)
- [Documentation index](../../../docs/en/README.md)
- [Repository maintenance rules](../../../AGENTS.md)
