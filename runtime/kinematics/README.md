# kinematics — pose geometry

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Computes geometric poses from an embedded MJCF description and joint angles, including head aiming. It compiles root-to-site chains once for repeated queries. It is a mathematical library, without servo I/O, ONNX inference, or contact dynamics.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/lib.rs](src/lib.rs) | Model parsing entry, compiled chains, sites, and pose queries |
| [src/mjcf.rs](src/mjcf.rs) | XML body/joint/site parsing |
| [src/math.rs](src/math.rs) | Pose/quaternion and transform helpers |
| [src/head.rs](src/head.rs) | Head aiming using the same geometry |
| [assets/alpha/robot_walk.xml](assets/alpha/robot_walk.xml) | Geometry embedded at compile time |

## Call and data flow

1. robotd uses geometric results for supported look/control computations. robotctl has its own embedded-mesh geometry code and does not link this crate.
2. A joint-angle slice and resolved site produce a transform through the precompiled chain.
3. The MJCF source is embedded; editing a board-side XML file alone does not replace already compiled geometry.

### Direct workspace relationships

- Local crate dependencies: None inside this workspace.
- Direct consumers: [robotd](../robotd/README.md)

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Geometry and joint naming must match the intended robot. XML ranges are not measured encoder calibration.
- This is not a MuJoCo simulator or training workspace. A legacy look API does not prove equivalent behavior in the installed XGO head-command path.

## Further reading

- [Detailed guide](../../docs/en/runtime.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
