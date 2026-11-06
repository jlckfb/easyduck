# IMU 驱动骨架

[English](README.md) | **简体中文**

## 职责

为正式控制与标定同时增加一种 IMU 提供最小 Rust／Python 框架。两端都要实现初始化和报文解析，当前骨架会明确返回未支持／未实现。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [imu-driver-template.rs](imu-driver-template.rs) | Rust Driver 实现框架及能力／读取／关闭方法 |
| [imu-driver-template.py](imu-driver-template.py) | Python 读取框架及能力／读取／关闭方法 |
| [../../runtime/duck-control/src/imu_driver.rs](../../runtime/duck-control/src/imu_driver.rs) | 正式 Rust 登记／处理约定 |
| [../../calibration/imu/__init__.py](../../calibration/imu/__init__.py) | 正式 Python 登记／处理 |

## 调用与数据流

1. 在实际芯片驱动中补齐有界传输、身份检查、报告解析和资源清理。
2. 在两端登记型号／配置，核对共用单位、四元数顺序、坐标、时间与标定行为。

## 边界与修改注意

- 机器人服务不编译或导入这些文件，直接执行不能读取传感器。
- 不要把装配旋转写死在芯片解析中，也不要把缺失测量补零；仍需实物轴向核对。

## 相关说明

- [详细使用说明](../../docs/zh-CN/imu-drivers.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
