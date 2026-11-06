# 公开配置模板

[English](README.md) | **简体中文**

## 职责

保存可安装的格式样例与硬件参考配置，不是出厂机器的实测标定。安装时明确选择配置类型，两套都关闭动作策略并标记安装／关节待标定；每台实物必须保存自身数据。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [examples](examples) | 通用待标定格式样例 |
| [board](board) | 泰山派硬件参考，同样待标定 |
| [board/robotd.toml](board/robotd.toml) | 串口／IMU／模型／音频／安全参考设置 |
| [board/visual_calibration.json](board/visual_calibration.json) | 关节／IMU 标定占位数据 |
| [board/zero_pos.json](board/zero_pos.json) | 标定与驱动回退使用的基线格式 |
| [board/scs_joint_gains.json](board/scs_joint_gains.json) | 关节增益参考与可选覆盖 |

## 调用与数据流

1. deploy/install.py 选择 examples 或 board，将配置放入 `/etc/robot/`，默认保留已有文件。
2. robotd 读取启动设置，标定读取相同 IMU 设置并保存本机 visual 标定；robotctl configure 按共享 Rust 定义编辑主控 TOML。
3. 音频启动配置读取 voice.env，以固定种子生成音效库。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [board](board/README.zh-CN.md) | 泰山派参考模板 |
| [examples](examples/README.zh-CN.md) | 通用配置模板 |

## 边界与修改注意

- HOME 2048、方向 −1 和编码器范围 0～4095 都是占位值。policy.enabled=false 不代表所有非策略操作都无法让舵机上力。
- 公开模板不放密码、密钥和单机备份。调整默认值不能覆盖已部署标定；板级与通用配置的安全开关不同，以实际 TOML 为准。

## 相关说明

- [详细使用说明](../docs/zh-CN/configuration.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
