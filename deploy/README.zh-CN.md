# 构建、安装与服务集成

[English](README.md) | **简体中文**

## 职责

把仓库源码衔接到应用程序、暂存文件系统和 Linux 服务管理。编译、安装、核对、回退、启动是分开的操作；SDK、内核与系统镜像构建仍在应用仓库之外。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [Dockerfile](Dockerfile) | 固定 Rust 工具链与交叉编译依赖 |
| [build.sh](build.sh) | 独立源码暂存与 ARM64 工作区编译 |
| [install.py](install.py) | 模型校验、安装映射、备份与安装文件校验 |
| [rollback.py](rollback.py) | 按清单恢复文件／链接／权限 |
| [start.sh](start.sh) | 预览或明确启动正式服务 |
| [dependencies.lock.json](dependencies.lock.json) | 参考工具链与系统／许可要求 |
| [network_routes.py](network_routes.py) | 共用可达地址与导航辅助 |
| [navigation-client.js](navigation-client.js) | 浏览器页面导航与地址交接 |
| [systemd](systemd) | 服务单元与覆盖配置 |
| [setup](setup) | Wi-Fi／蓝牙设置与服务交接 |
| [kernel](kernel) | 板级内核输入与镜像侧手柄检查 |

## 调用与数据流

1. build.sh 使用已有编译容器生成四个程序，不启动板端服务。
2. install.py 把程序、已校验模型、公开配置、资源、服务素材和许可记录组合到指定根目录，除非明确替换，否则保留已有配置。
3. start.sh 只在执行模式下创建系统身份并启动正式服务；回退按清单恢复文件，不自动启动。SDK 集成另行使用内核／手柄输入。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [alsa](alsa/README.zh-CN.md) | 板端声卡状态 |
| [kernel](kernel/README.zh-CN.md) | 板级内核集成输入 |
| [modules-load.d](modules-load.d/README.zh-CN.md) | 手柄开机加载规则 |
| [network-access](network-access/README.zh-CN.md) | 本地名称与 IP 播报 |
| [setup](setup/README.zh-CN.md) | Wi-Fi／蓝牙设置服务 |
| [systemd](systemd/README.zh-CN.md) | Linux 服务单元与覆盖配置 |
| [sysusers.d](sysusers.d/README.zh-CN.md) | 应用访问用系统身份 |

## 边界与修改注意

- 应用安装不能补齐缺失的内核驱动、ONNX Runtime 或全部系统软件包，rootfs 检查也不是刷机、开机或实物验收。
- 实机安装、重启、替换配置和回退需要相应授权与机器人支撑。不能把用过机器的标定或凭据打包成默认值。

## 相关说明

- [详细使用说明](../docs/zh-CN/deployment.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
