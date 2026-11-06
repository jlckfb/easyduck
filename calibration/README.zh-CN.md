# 舵机与 IMU 标定服务

[English](README.md) | **简体中文**

## 职责

运行独立的硬件标定工作台，管理舵机扫描／改号、硬件中位、HOME、方向／范围、运动任务、IMU 安装／偏置、草稿与保存。标定运行期间，由 Python 后端直接占用硬件。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [server.py](server.py) | HTTP／API 入口、应用状态、校验和退出协调 |
| [hardware.py](hardware.py) | 实物串口总线与 MockBus 实现 |
| [motion.py](motion.py) | 有界运动任务与停止处理 |
| [servo_setup.py](servo_setup.py) | ID 扫描、写入、核对和记录 |
| [calibration_math.py](calibration_math.py) | 关节／参考姿态与标定数学 |
| [imu_settings.py](imu_settings.py) | 共用 robotd.toml 的 IMU 选型校验与保存 |
| [imu](imu) | 芯片驱动、传输与共享 IMU 处理 |
| [static](static) | 工作台界面、3D 网格与浏览器依赖 |
| [dependencies.lock.json](dependencies.lock.json) | 浏览器依赖版本与哈希 |

## 调用与数据流

1. 工作台向 server.py 发送 JSON 命令，后端调用 hardware、motion、servo_setup 及共享应用状态。
2. IMU 包读取与 robotd 相同的硬件配置，并按型号应用安装与偏置标定。
3. 登录后的设置导航在标定前停止正式服务；退出时检查草稿／运动／力矩，关闭硬件后再让正式服务读取已保存设置。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [imu](imu/README.zh-CN.md) | 标定用 IMU 读取模块 |
| [static](static/README.zh-CN.md) | 标定台浏览器前端 |

## 边界与修改注意

- 硬件中位写入是舵机永久操作，与记录 HOME、保存软件草稿不同。写入前必须确认对应的实物关节。
- 无硬件预览使用 --mock。8766 接口没有独立账号认证；服务停止或导航成功也不能证明实物已断力。

## 相关说明

- [详细使用说明](../docs/zh-CN/calibration.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
