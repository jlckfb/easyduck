# 通用配置模板

[English](README.md) | **简体中文**

## 职责

通过 --profile examples 选择的可安装待标定配置，策略默认关闭，实物关节／IMU 标定尚未完成。通用配置没有显式启用板级低电量坐下。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [robotd.toml](robotd.toml) | 硬件、策略、音频与保护设置 |
| [visual_calibration.json](visual_calibration.json) | 关节／IMU 占位数据与待标定标记 |
| [zero_pos.json](zero_pos.json) | 基线格式与回退限位 |
| [scs_joint_gains.json](scs_joint_gains.json) | 增益参考与覆盖 |
| [calibration_speed_restore.json](calibration_speed_restore.json) | 舵机速度恢复值 |
| [voice.env](voice.env) | 确定性音效种子 |

## 调用与数据流

1. install.py 将整套配置安装到 /etc/robot，默认保留目标已有文件。
2. robotd 与标定读取各自需要的设置，动作前须生成本机实测标定。

## 边界与修改注意

- 占位 HOME／方向／范围及单位 IMU 安装旋转不是实测标定，不能将另一台机器的数据复制到这里。
- 这里的改动影响安装输入，不会立即修改运行中的主控；保留另一套配置，并排除私有凭据。

## 相关说明

- [详细使用说明](../../docs/zh-CN/configuration.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
