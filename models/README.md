# 动作模型

本目录提供泰山派 1M＋HD1910 主控使用的五个 ONNX 模型。模型已包含输入输出方向适配。其中四个模型保留上游原始模型的学习权重，坐下／起立模型为自训模型；具体来源见 `manifest.json`。

## 模型与配置

| 文件 | `robotd.toml` 配置键 | 用途 |
| --- | --- | --- |
| `xgoduck_walk_original_signs_20261009.onnx` | `policy.walk`、`policy.stand` | 行走与零速站立，共用一个模型 |
| `xgoduck_getup_official_tspi_20261013.onnx` | `policy.sitstand` | 起身网络 |
| `xgoduck_posture_14999_tspi.onnx` | `policy.posture` | 手柄指令坐下／起立，独立于倒地恢复 |
| `xgoduck_pick_official_tspi_20261013.onnx` | `policy.ground_pick` | 捡物动作网络 |
| `xgoduck_roulade_official_tspi_20261013.onnx` | `policy.roulade` | 翻滚动作网络 |

安装位置为 `/opt/robot/daemon/policies/`，文件名与配置中的路径对应。模型由主控加载和切换，不单独运行。完整参考配置见 [`config/examples/robotd.toml`](../config/examples/robotd.toml)。

`manifest.json` 是来源和接口清单：记录部署文件及上游原始文件的完整 SHA256、固定上游版本、下载地址、模型元数据、方向适配矩阵和配置对应关系。`SHA256SUMS` 可用于直接核对文件是否完整：

```sh
cd models
sha256sum -c SHA256SUMS
```

## 输入输出

输入 `obs` 为 float32 `[1,61]`，输出 `actions` 为 float32 `[1,14]`。14 个策略关节的顺序如下，嘴部 34 号由主控单独控制：

```text
20 21 22 23 24 30 31 32 33 10 11 12 13 14
```

输入切片为左闭右开：

| 输入范围 | 内容 |
| --- | --- |
| `0:3` | 躯干坐标系角速度 |
| `3:6` | 投影重力，直立时约为 `[0,0,-1]` |
| `6:20` | 关节相对主控基线的位置 |
| `20:34` | 关节速度 |
| `34:48` | 上一次模型输出 |
| `48:51` | 移动指令：前后、左右、转向 |
| `51:55` | 头部指令：颈俯仰、头俯仰、偏航、侧摆 |
| `55:61` | 身体指令：x、y、z、roll、pitch、yaw；当前主控的 x、y、yaw 固定为 0 |

## 方向与控制约定

- `J` 在 30、31 号关节为 −1，其余为 +1；`S` 为驱动使用的标定方向。本组模型按各策略关节的 `S = −1` 适配。
- 模型内部对位置乘 `J`、速度乘 `J*S`、上一次动作乘 `J`、输出乘 `J`。主控已有的观测和目标转换仍按其接口执行，不能再额外套同样的模型适配。
- `policy.action_scale` 使用 `1.0`；观测归一化已包含在模型内。
- 模型元数据中的默认姿态不能直接作为实物编码器零点。其他机器人需先完成自身标定，并核对相同的关节顺序与方向约定。

## 来源

原始模型来自 [LuwuDynamics/xgoduck_runtime_arduino 的固定版本](https://github.com/LuwuDynamics/xgoduck_runtime_arduino/tree/8cdbbd84710d856581982c9eaf0d5e2970666232)。逐个文件的下载地址和原始哈希见 `manifest.json`。

该固定版本未附明确的许可证文件；这些模型不因本仓库根目录的许可证而自动获得相同授权。

## 自训坐起模型

`xgoduck_posture_14999_tspi.onnx` 来自官方 `Mjlab-SitStand-Flat-XgoDuck` 任务的最终检查点 `model_14999.pt`。训练源码固定为 `afd091be041caf5e6d4f44102c959e7571b82a37`，使用官方物理与奖励配置，并行环境数为2048。检查点、原始导出及部署模型的哈希见 manifest。

该模型通过移动指令块的第一项接收姿态标志：1为坐下，0为起立；其余移动项为零。这是专用动作阶段的编码，不是行走速度。输入归一化在模型内，方向适配与主控契约一致。此模型适用本仓库 Apache-2.0；上面所述另四个模型仍保留各自来源与授权状态。
