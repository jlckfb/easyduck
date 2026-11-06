# ONNX policies and provenance

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Bundles five inference models plus their interface, origin, and hash records. It does not contain training code or a model server. The daemon selects policies through configuration; walking/standing share one network, with separate get-up, commanded posture, pickup, and roulade roles.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [manifest.json](manifest.json) | Pinned upstream/training provenance, deployed/original hashes, metadata, direction adaptation |
| [SHA256SUMS](SHA256SUMS) | Direct integrity check list |
| [xgoduck_walk_original_signs_20261009.onnx](xgoduck_walk_original_signs_20261009.onnx) | Shared walk/stand policy |
| [xgoduck_getup_official_tspi_20261013.onnx](xgoduck_getup_official_tspi_20261013.onnx) | Fall-recovery get-up policy |
| [xgoduck_posture_14999_tspi.onnx](xgoduck_posture_14999_tspi.onnx) | Separately trained commanded sit/stand policy |
| [xgoduck_pick_official_tspi_20261013.onnx](xgoduck_pick_official_tspi_20261013.onnx) | Pickup policy |
| [xgoduck_roulade_official_tspi_20261013.onnx](xgoduck_roulade_official_tspi_20261013.onnx) | Roulade policy |

## Call and data flow

1. deploy/install.py verifies each manifest-listed file hash before copying models to the application policies directory.
2. robotd-params resolves configured slots; robotd action state selects a policy; duck-control constructs the observation and runs inference.
3. The observation has 61 values and the output 14 actions; mouth ID34 is separate. Physical target conversion uses the robot's calibration.

## Boundaries and change notes

- Preserve observation/action order, units, model direction adaptation, and action scaling. Model reference angles are not physical encoder HOME.
- Four upstream models have unresolved redistribution terms in recorded sources; the separately trained posture model has its own provenance/license. A hash pass does not grant redistribution rights or validate motion.

## Further reading

- [Detailed guide](../docs/en/models.md)
- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
