# 标定用 IMU 读取模块

[English](README.md) | **简体中文**

## 职责

标定服务使用的 Python 传感器驱动与共用姿态处理。芯片层报告本次新收到的数据，共用层校验配置、跟踪能力／时效并应用按型号保存的安装与偏置；遵守 Rust 端数据约定，但不调用 Rust 驱动。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [__init__.py](__init__.py) | 型号登记、打开驱动、共用处理与标定选择 |
| [transport.py](transport.py) | Linux I2C 描述符与事务处理 |
| [lsm6dsv16x_reader.py](lsm6dsv16x_reader.py) | LSM 报告与支持的融合模式 |
| [bno085.py](bno085.py) | 独立 BNO SH-2／SHTP 解析 |
| [../imu_settings.py](../imu_settings.py) | robotd.toml 配置选择与保存 |

## 调用与数据流

1. calibration/server.py 打开选定驱动，将报告经共享处理层送入工作台状态与标定采样。
2. 保存文件按型号记录安装／偏置，robotd 后续通过 Rust 实现读取匹配的标定。

## 边界与修改注意

- 缺失数据不能补成虚假的零值或刷新时间戳。四元数顺序、单位、躯干坐标与时间处理必须和 Rust 一致。
- 同一设备不能同时被本模块和 robotd 读取。保留原 GPL 署名，新芯片支持仍需实物核对。

## 相关说明

- [详细使用说明](../../docs/zh-CN/imu-drivers.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
