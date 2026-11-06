# EasyDuck 文档

[English](../en/README.md) | **简体中文**

首次使用先读软件使用说明，编译细节看部署文档，新机器动作前先完成标定。文档中的部分命令会影响实物，请遵守对应操作条件。当前网页界面仍为中文。

| 文档 | 内容 |
| --- | --- |
| [软件使用说明](software-guide.md) | 安装、配置、启动、日常操作与回退 |
| [编译与部署](deployment.md) | 工具链、暂存安装、文件检查与回退 |
| [连接设置](connection-setup.md) | Wi-Fi、PS5 配对与服务交接 |
| [网络访问与 IP 播报](network-access.md) | 手机热点、局域网名称和地址播报 |
| [USB / 2.4G 手柄](usb-gamepad.md) | 接收器、xpad 与 SDK 镜像要求 |
| [网页控制](web-control.md) | 操作、状态、离线测试和页面切换 |
| [舵机与 IMU 标定](calibration.md) | ID、硬件中位、HOME、方向、范围与 IMU |
| [配置说明](configuration.md) | 配置模板、优先级、标定文件和音效种子 |
| [动作模型](models.md) | 模型用途、输入输出约定和来源 |
| [低电量保护](low-battery.md) | 门槛、坐下与断力流程和验证范围 |
| [机器人音效](sounds.md) | 音效库生成、导出和播放 |
| [Rust 运行程序](runtime.md) | 程序、共用模块与运行边界 |
| [命令行工具](robotctl.md) | 只读监控、参数编辑与控制命令 |
| [IMU 驱动接入](imu-drivers.md) | 数据约定、型号支持、标定和扩展 |

## 目录与模块介绍

各目录 README 直接说明职责、实际文件、调用关系和修改边界。

| 目录 | 介绍 |
| --- | --- |
| [LICENSES](../../LICENSES/README.zh-CN.md) | 原始许可证正文 |
| [assets](../../assets/README.zh-CN.md) | 仓库展示素材 |
| [assets/demos](../../assets/demos/README.zh-CN.md) | 实物动作展示动图 |
| [assets/images](../../assets/images/README.zh-CN.md) | 实物照片位置 |
| [calibration](../../calibration/README.zh-CN.md) | 舵机与 IMU 标定服务 |
| [calibration/imu](../../calibration/imu/README.zh-CN.md) | 标定用 IMU 读取模块 |
| [calibration/static](../../calibration/static/README.zh-CN.md) | 标定台浏览器前端 |
| [calibration/static/model](../../calibration/static/model/README.zh-CN.md) | 标定 3D 网格 |
| [config](../../config/README.zh-CN.md) | 公开配置模板 |
| [config/board](../../config/board/README.zh-CN.md) | 泰山派参考模板 |
| [config/examples](../../config/examples/README.zh-CN.md) | 通用配置模板 |
| [deploy](../../deploy/README.zh-CN.md) | 构建、安装与服务集成 |
| [deploy/alsa](../../deploy/alsa/README.zh-CN.md) | 板端声卡状态 |
| [deploy/kernel](../../deploy/kernel/README.zh-CN.md) | 板级内核集成输入 |
| [deploy/modules-load.d](../../deploy/modules-load.d/README.zh-CN.md) | 手柄开机加载规则 |
| [deploy/network-access](../../deploy/network-access/README.zh-CN.md) | 本地名称与 IP 播报 |
| [deploy/setup](../../deploy/setup/README.zh-CN.md) | Wi-Fi／蓝牙设置服务 |
| [deploy/setup/static](../../deploy/setup/static/README.zh-CN.md) | 连接设置浏览器资源 |
| [deploy/systemd](../../deploy/systemd/README.zh-CN.md) | Linux 服务单元与覆盖配置 |
| [deploy/sysusers.d](../../deploy/sysusers.d/README.zh-CN.md) | 应用访问用系统身份 |
| [docs](../README.zh-CN.md) | 双语项目文档 |
| [examples](../../examples/README.zh-CN.md) | 开发示例 |
| [examples/imu](../../examples/imu/README.zh-CN.md) | IMU 驱动骨架 |
| [models](../../models/README.zh-CN.md) | ONNX 动作模型与来源 |
| [runtime](../../runtime/README.zh-CN.md) | Rust 运行工作区 |
| [runtime/duck-control](../../runtime/duck-control/README.zh-CN.md) | duck-control — 硬件与推理共用库 |
| [runtime/duck-ipc-proto](../../runtime/duck-ipc-proto/README.zh-CN.md) | duck-ipc-proto — 共用消息约定 |
| [runtime/kinematics](../../runtime/kinematics/README.zh-CN.md) | kinematics — 姿态几何计算 |
| [runtime/padd](../../runtime/padd/README.zh-CN.md) | padd — 手柄输入客户端 |
| [runtime/robotctl](../../runtime/robotctl/README.zh-CN.md) | robotctl — 命令行操作 |
| [runtime/robotd](../../runtime/robotd/README.zh-CN.md) | robotd — 机器人主控程序 |
| [runtime/robotd-params](../../runtime/robotd-params/README.zh-CN.md) | robotd-params — 共用配置定义 |
| [runtime/sounds](../../runtime/sounds/README.zh-CN.md) | sounds — 可重复音效合成 |
| [runtime/test-fixtures](../../runtime/test-fixtures/README.zh-CN.md) | 默认参数测试输入 |
| [tools](../../tools/README.zh-CN.md) | 仓库维护检查 |
| [web](../../web/README.zh-CN.md) | 浏览器控制页与网关 |
| [web/assets](../../web/assets/README.zh-CN.md) | 控制页浏览器资源 |

## 开发示例

[IMU 驱动模板](../../examples/imu/README.zh-CN.md)是未完成的开发示例，不随机器人服务运行。

## 项目与维护

[项目介绍与架构](../../README.zh-CN.md) · [仓库维护规则](../../AGENTS.md) · [文档规则](../AGENTS.md) · [第三方声明](../../THIRD_PARTY_NOTICES.md)

许可证和第三方声明保留原文，不另行翻译出一套授权条款。离线检查不能代替实物动作验收。
