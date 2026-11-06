# Calibration browser frontend

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Displays calibration state and translates deliberate user actions into backend commands. The shared state feed serves all panels; 3D rendering uses local meshes/libraries. Hardware access and final validation remain in the Python service.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [index.html](index.html) | Workbench panels and controls |
| [app.js](app.js) | Joint/whole-body/IMU workflows and 3D integration |
| [state-feed.js](state-feed.js) | Shared state polling, serialized command state, no automatic write retry |
| [servo-id.js](servo-id.js) | Servo discovery/ID-assignment UI |
| [calibration-file.js](calibration-file.js) | Import JSON shape/size parsing before backend validation |
| [appearance.js](appearance.js) | Theme persistence and connection indicators |
| [style.css](style.css) | Workbench layout and visual style |
| [model](model) | Mesh files, mapping, and provenance |
| [vendor](vendor) | Vendored three.js modules and original license |

## Call and data flow

1. Browser code reads /api/state through state-feed.js and submits explicit operations to /api/command.
2. server.py validates operations and performs bus/IMU actions; returned snapshots update panels and 3D without giving JavaScript hardware ownership.

## Subdirectories

| Directory | Description |
| --- | --- |
| [model](model/README.md) | Calibration 3D meshes |

## Boundaries and change notes

- A frontend precheck cannot replace backend validation. A timed-out write must be resolved from actual state before another request.
- The 3D view is visualization, not physical simulation. Keep vendor code/licenses and mesh provenance intact; UI labels currently remain Chinese.

## Further reading

- [Detailed guide](../../docs/en/calibration.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
