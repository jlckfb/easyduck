# Motion policies

**English** | [简体中文](../zh-CN/models.md)

`models/` contains five ONNX policies for the TaishanPi 1M + HD1910 daemon, including input/output direction adaptation. Four retain upstream learned weights; the commanded sit/stand policy was trained separately. See `models/manifest.json` for provenance.

## Models and configuration

| File | `robotd.toml` key | Purpose |
| --- | --- | --- |
| `xgoduck_walk_original_signs_20261009.onnx` | `policy.walk`, `policy.stand` | Shared walking / zero-speed standing policy |
| `xgoduck_getup_official_tspi_20261013.onnx` | `policy.sitstand` | Get-up network |
| `xgoduck_posture_14999_tspi.onnx` | `policy.posture` | Commanded sit/stand, separate from fall recovery |
| `xgoduck_pick_official_tspi_20261013.onnx` | `policy.ground_pick` | Pickup network |
| `xgoduck_roulade_official_tspi_20261013.onnx` | `policy.roulade` | Roulade network |

Installation uses `/opt/robot/daemon/policies/`; configured paths must match. The daemon loads/switches policies; they do not run independently. See the [reference configuration](../../config/examples/robotd.toml).

`models/manifest.json` records complete SHA256 values for deployed/upstream files, pinned upstream revisions, download URLs, metadata, direction matrices, and configuration mappings. Check file integrity from the repository root:

```sh
cd models
sha256sum -c SHA256SUMS
```

## Inputs and outputs

Input `obs` is float32 `[1,61]`; output `actions` is float32 `[1,14]`. Policy joints are ordered as below; mouth ID34 is separate:

```text
20 21 22 23 24 30 31 32 33 10 11 12 13 14
```

Slices include the left bound and exclude the right:

| Slice | Content |
| --- | --- |
| `0:3` | Torso-frame angular velocity |
| `3:6` | Projected gravity, approximately `[0,0,-1]` upright |
| `6:20` | Joint positions relative to the daemon baseline |
| `20:34` | Joint velocities |
| `34:48` | Previous policy output |
| `48:51` | Movement: forward/backward, lateral, yaw |
| `51:55` | Head: neck pitch, head pitch, yaw, roll |
| `55:61` | Body: x, y, z, roll, pitch, yaw; current daemon fixes x, y, yaw at 0 |

## Direction and control conventions

- `J` is −1 at IDs30/31 and +1 elsewhere; `S` is the driver's calibration direction. These models are adapted for `S=−1` at every policy joint.
- Inside the model, positions multiply by `J`, velocities by `J*S`, previous actions by `J`, and outputs by `J`. Existing daemon observation/target conversions still apply; do not add the same model adaptation again.
- `policy.action_scale` is `1.0`; observation normalization is embedded in the model.
- Metadata nominal poses are not physical encoder HOME. Calibrate each robot and verify the same joint order/direction conventions.

## Provenance

Original models come from a [pinned LuwuDynamics/xgoduck_runtime_arduino revision](https://github.com/LuwuDynamics/xgoduck_runtime_arduino/tree/8cdbbd84710d856581982c9eaf0d5e2970666232). Per-file URLs and original hashes are in `models/manifest.json`.

That revision has no explicit license file. The repository root license does not automatically license these models.

## Separately trained sit/stand policy

`xgoduck_posture_14999_tspi.onnx` comes from final checkpoint `model_14999.pt` of the official `Mjlab-SitStand-Flat-XgoDuck` task. Training source is pinned to `afd091be041caf5e6d4f44102c959e7571b82a37`, using official physics/rewards and 2048 parallel environments. The manifest records checkpoint, original-export, and deployed-model hashes.

The first movement-command field encodes posture: 1 for sitting, 0 for standing; other movement fields are zero. This is action-phase encoding, not walking velocity. Normalization is embedded and direction adaptation matches the daemon contract. This model uses the repository's Apache-2.0 license; the other four retain their own recorded provenance and unresolved licensing status.
