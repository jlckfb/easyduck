# duck-ipc-proto — 共用消息约定

[English](README.md) | **简体中文**

## 职责

定义机器人服务与客户端交换的数据：JSON-RPC 消息外壳、方法名称、请求与结果结构、错误码、遥测及共用关节名称。这里提供类型和编码／解析约定，不提供服务进程或硬件传输循环。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [Cargo.toml](Cargo.toml) | 轻量序列化依赖与可选 test-support 功能 |
| [src/lib.rs](src/lib.rs) | 协议常量、socket 名、Call／Request／Response／Error 和方法数据 |
| [../robotd/src/main.rs](../robotd/src/main.rs) | 主控侧消息解析与方法处理使用方 |
| [../padd/src/main.rs](../padd/src/main.rs) | 手柄请求发送方 |
| [../robotctl/src/main.rs](../robotctl/src/main.rs) | 命令行请求发送与结果读取方 |

## 调用与数据流

1. padd 和 robotctl 组装有类型的请求，robotd 解析并通过调用方管理的 Unix socket 返回结果或错误。
2. duck-control 复用关节名称，避免遥测数组与模型／驱动命名各自漂移。
3. Python 网关另行实现兼容的线上消息；只改 Rust 类型不会同步修改 Python 调用方。

### 工作区直接关系

- 依赖的本地库：工作区内没有。
- 直接使用方：[duck-control](../duck-control/README.zh-CN.md)、[padd](../padd/README.zh-CN.md)、[robotctl](../robotctl/README.zh-CN.md)、[robotd](../robotd/README.zh-CN.md)、[robotd-params](../robotd-params/README.zh-CN.md)

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 兼容性包含字段含义和数组顺序，不只是 JSON 能否解析。修改时要同时检查发送方和接收方。
- 保留的 configd／updaterd 方法与类型不等于实现了这些外部服务，不能把它们的默认 socket 常量误当作 robotd 的路径。

## 相关说明

- [详细使用说明](../../docs/zh-CN/runtime.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
