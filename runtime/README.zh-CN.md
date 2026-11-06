# Rust 运行工作区

[English](README.md) | **简体中文**

## 职责

包含四个可执行程序和四个共用库，Cargo 一起管理它们的本地依赖。正式控制链是客户端请求进入 robotd，再进行硬件读写与推理；共用库不会各自启动服务。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [Cargo.toml](Cargo.toml) | 工作区成员、默认程序及共用 Rust 版本／依赖 |
| [Cargo.lock](Cargo.lock) | 锁定的依赖解析结果 |
| [robotd](robotd) | 主控制与硬件占用 |
| [padd](padd) | 手柄意图客户端 |
| [robotctl](robotctl) | 终端操作与监控 |
| [sounds](sounds) | 音效生成程序与库 |
| [duck-control](duck-control) | 硬件、传感器、模型输入输出与推理 |
| [duck-ipc-proto](duck-ipc-proto) | 共享消息与关节名称定义 |
| [robotd-params](robotd-params) | 共享启动配置／默认值／校验 |
| [kinematics](kinematics) | 几何姿态计算 |
| [test-fixtures](test-fixtures) | 测试配置输入，不是机器人默认配置 |

## 调用与数据流

1. padd 和 robotctl 提交本机请求，目录外的 Python 网页服务使用相同主控入口。
2. robotd 链接控制、协议、参数、几何与音频库；下面的依赖表按当前 Cargo 清单生成。
3. deploy/build.sh 交叉编译程序，deploy/install.py 将程序与独立维护的模型、配置一起安装。

### 工作区依赖图

| 成员 | 直接本地依赖 |
| --- | --- |
| [robotd](robotd/README.zh-CN.md) | `duck-control`, `duck-ipc-proto`, `kinematics`, `robotd-params`, `sounds` |
| [padd](padd/README.zh-CN.md) | `duck-ipc-proto` |
| [robotctl](robotctl/README.zh-CN.md) | `duck-ipc-proto`, `robotd-params` |
| [sounds](sounds/README.zh-CN.md) | 无 |
| [duck-control](duck-control/README.zh-CN.md) | `duck-ipc-proto` |
| [duck-ipc-proto](duck-ipc-proto/README.zh-CN.md) | 无 |
| [robotd-params](robotd-params/README.zh-CN.md) | `duck-ipc-proto` |
| [kinematics](kinematics/README.zh-CN.md) | 无 |

运行时还通过 IPC 把 padd／robotctl／网页连接到 robotd；socket 调用不等于 Cargo 依赖。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [duck-control](duck-control/README.zh-CN.md) | duck-control — 硬件与推理共用库 |
| [duck-ipc-proto](duck-ipc-proto/README.zh-CN.md) | duck-ipc-proto — 共用消息约定 |
| [kinematics](kinematics/README.zh-CN.md) | kinematics — 姿态几何计算 |
| [padd](padd/README.zh-CN.md) | padd — 手柄输入客户端 |
| [robotctl](robotctl/README.zh-CN.md) | robotctl — 命令行操作 |
| [robotd](robotd/README.zh-CN.md) | robotd — 机器人主控程序 |
| [robotd-params](robotd-params/README.zh-CN.md) | robotd-params — 共用配置定义 |
| [sounds](sounds/README.zh-CN.md) | sounds — 可重复音效合成 |
| [test-fixtures](test-fixtures/README.zh-CN.md) | 默认参数测试输入 |

## 边界与修改注意

- 源码、已部署程序和生成的系统镜像是不同状态，编译不会自动部署或启动服务。
- 固定样例与 FakeIo 用于软件检查，不是接触仿真或实物动作验收；修改接口前阅读各成员说明。

## 相关说明

- [详细使用说明](../docs/zh-CN/runtime.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
