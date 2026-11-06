# robotd-params — shared configuration schema

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Owns startup parameter types, defaults, validation, policy path resolution, and editable-field metadata. Sharing it lets robotd read a configuration that robotctl configure edits using the same schema, rather than maintaining two independent sets of defaults.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [src/lib.rs](src/lib.rs) | Params, IMU/policy/safety/bus/audio settings and validation |
| [src/registry.rs](src/registry.rs) | Editable field kinds, units, labels, and lookup |
| [../robotd/src/params.rs](../robotd/src/params.rs) | Re-export used by the daemon |
| [../robotctl/src/configure.rs](../robotctl/src/configure.rs) | TOML editor consuming schema and registry |
| [../../config/examples/robotd.toml](../../config/examples/robotd.toml) | Public example profile |

## Call and data flow

1. robotd loads and validates startup settings before constructing runtime objects.
2. robotctl configure uses the registry for its editing interface and validates through shared definitions before saving.
3. The Python calibration reader separately consumes robotd.toml; IMU schema changes must stay compatible on both sides.

### Direct workspace relationships

- Local crate dependencies: [duck-ipc-proto](../duck-ipc-proto/README.md)
- Direct consumers: [robotctl](../robotctl/README.md), [robotd](../robotd/README.md)

These are Cargo dependency edges, not proof that every optional runtime path is active.

## Boundaries and change notes

- Changing defaults affects new/omitted fields, not necessarily explicit values on existing robots. Do not overwrite private calibration while changing a public example.
- A declared parameter or optional subsystem is not proof that the active XGO path implements it. Synchronize schema, editor metadata, templates, and affected callers.

## Further reading

- [Detailed guide](../../docs/en/configuration.md)
- [Documentation index](../../docs/en/README.md)
- [Repository maintenance rules](../../AGENTS.md)
