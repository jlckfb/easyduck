# 开发示例

[English](README.md) | **简体中文**

## 职责

集中存放与正式实现分开的开发骨架，用于说明扩展约定，不会作为活动驱动安装或加入正常 Cargo 工作区。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [imu](imu) | 成对的 Rust／Python 骨架及双语说明 |
| [../runtime/duck-control/src/imu_driver.rs](../runtime/duck-control/src/imu_driver.rs) | 实际 Rust 扩展约定 |
| [../calibration/imu](../calibration/imu) | 实际 Python 读取实现 |

## 调用与数据流

1. 开发者以示例为起点补齐设备行为，再到应用模块登记实际驱动。
2. 应用使用实际驱动目录，不调用本示例目录。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [imu](imu/README.zh-CN.md) | IMU 驱动骨架 |

## 边界与修改注意

- 未完成示例不能证明已支持某硬件，应明确失败，不能编造传感器数据。
- 示例不放私有配置或生成产物，扩展约定变化时同步两种语言。

## 相关说明

- [详细使用说明](../docs/zh-CN/imu-drivers.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
