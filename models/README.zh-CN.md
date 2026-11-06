# ONNX 动作模型与来源

[English](README.md) | **简体中文**

## 职责

包含五个推理模型及接口、来源、哈希记录，不包含训练代码或模型服务。主控按配置选择模型：行走与零速站立共用网络，另有起身、指令坐起、捡物和翻滚用途。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [manifest.json](manifest.json) | 固定上游／训练来源、部署／原始哈希、元数据和方向适配 |
| [SHA256SUMS](SHA256SUMS) | 直接文件完整性校验清单 |
| [xgoduck_walk_original_signs_20261009.onnx](xgoduck_walk_original_signs_20261009.onnx) | walk／stand 共用策略 |
| [xgoduck_getup_official_tspi_20261013.onnx](xgoduck_getup_official_tspi_20261013.onnx) | 倒地恢复起身策略 |
| [xgoduck_posture_14999_tspi.onnx](xgoduck_posture_14999_tspi.onnx) | 单独训练的指令坐下／起立策略 |
| [xgoduck_pick_official_tspi_20261013.onnx](xgoduck_pick_official_tspi_20261013.onnx) | 捡物策略 |
| [xgoduck_roulade_official_tspi_20261013.onnx](xgoduck_roulade_official_tspi_20261013.onnx) | 翻滚策略 |

## 调用与数据流

1. deploy/install.py 核对清单中每个模型的哈希，再复制到应用模型目录。
2. robotd-params 解析配置项，robotd 动作状态选择模型，duck-control 构造观测并执行推理。
3. 输入为 61 个值、输出为 14 个动作，嘴部 ID34 独立控制；实物目标换算使用本机标定。

## 边界与修改注意

- 保持观测／动作顺序、单位、模型方向适配和动作缩放一致。模型参考角不是实物编码器 HOME。
- 四个上游模型的记录来源仍有再分发许可未明确项，自训姿态模型有独立来源／许可。哈希通过不等于取得再分发授权或完成动作验收。

## 相关说明

- [详细使用说明](../docs/zh-CN/models.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
