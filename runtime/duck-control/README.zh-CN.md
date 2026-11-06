# duck-control — 硬件与推理共用库

[English](README.md) | **简体中文**

## 职责

编入 robotd 的共用库，不是独立服务。它定义机器人 I/O 边界，提供舵机与模型坐标转换、IMU 读取、策略观测与推理，以及可复用的目标保护。Linux 硬件实现和模拟 I/O 共享接口。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/lib.rs](src/lib.rs) | 公开模块与导出入口 |
| [src/io.rs](src/io.rs) | RobotIo、Sensors、JointTargets 与 FakeIo 抽象 |
| [src/scs.rs](src/scs.rs) | ScsIo 串口报文、关节标定换算、反馈及力矩操作 |
| [src/model.rs](src/model.rs) | 关节 ID／顺序、模型参考姿态、方向与范围 |
| [src/obs.rs](src/obs.rs) | 61 维观测与 14 维动作顺序，不含嘴部 |
| [src/policy.rs](src/policy.rs) | 动态加载 ONNX Runtime、创建会话与推理 |
| [src/imu_driver.rs](src/imu_driver.rs) | 共享驱动约定、带时间的报告与躯干坐标处理 |
| [src/imu_i2c.rs](src/imu_i2c.rs) | LSM6DSV16X 读取 |
| [src/imu_bno085.rs](src/imu_bno085.rs) | BNO085 SH-2／SHTP 实现 |
| [src/imu_transport.rs](src/imu_transport.rs) | 有界 Linux I2C 传输 |
| [src/protection.rs](src/protection.rs) | 可复用命令与目标保护 |
| [src/safety.rs](src/safety.rs) | 安全辅助逻辑 |
| [src/fall.rs](src/fall.rs) | 倒地状态辅助逻辑 |
| [src/io_trace.rs](src/io_trace.rs) | 可选反馈／推理／总线记录 |

## 调用与数据流

1. robotd 创建选定的 I/O 与模型对象并管理生命周期，本库不启动 IPC 服务。
2. 舵机反馈和 IMU 快照构造成观测，推理产生模型动作，再换算成标定后的舵机目标。
3. 关节名称从 duck-ipc-proto 共用。标定台使用遵守相同 IMU 约定的 Python 实现，不直接导入这个 Rust 库。

### 工作区直接关系

- 依赖的本地库：[duck-ipc-proto](../duck-ipc-proto/README.zh-CN.md)
- 直接使用方：[robotd](../robotd/README.zh-CN.md)

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- ONNX Runtime 在需要时动态加载；编译成功不代表目标系统的运行库和模型可用。
- 保持关节顺序、单位、标定方向、数据时效和模型约定一致。FakeIo 与记录回放不模拟真实接触，也不证明实物动作；辅助函数存在不代表每条控制路径都调用它。

## 相关说明

- [详细使用说明](../../docs/zh-CN/models.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
