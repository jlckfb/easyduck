# robotctl — 命令行操作

[English](README.md) | **简体中文**

## 职责

用于状态查询、监控、控制请求和本地 TOML 编辑的终端客户端。服务实现不放在 CLI 中，服务不可达与请求被拒绝分别处理。终端 3D 展示收到的反馈，不是在仿真机器人运动。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/main.rs](src/main.rs) | 命令解析、服务路由、IPC 与返回码 |
| [src/configure.rs](src/configure.rs) | 保留注释与顺序的交互配置编辑 |
| [src/monitor.rs](src/monitor.rs) | 终端／JSON 遥测、手柄输入流与 3D 显示 |
| [src/duck.rs](src/duck.rs) | 内嵌鸭子显示几何支持 |
| [src/show.rs](src/show.rs) | 状态显示辅助 |
| [assets/duck.bin](assets/duck.bin) | 编译时终端显示网格，不是动作策略 |

## 调用与数据流

1. 机器人命令与遥测通过 robotd；configure 使用 robotd-params 校验后编辑本地文件。
2. 监控除读取主控遥测外，还可订阅 padd 的只读原始事件流。
3. 保留的网络／系统／配对及更新客户端调用外部 configd／updaterd，这些服务未包含在仓库中。

### 工作区直接关系

- 依赖的本地库：[duck-ipc-proto](../duck-ipc-proto/README.zh-CN.md)、[robotd-params](../robotd-params/README.zh-CN.md)
- 直接使用方：工作区内没有。

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 只读查询与动作命令的后果不同，init、relax、动作和合唱各有操作条件。
- 保存参数不会立即重配运行中的主控。重启服务可能影响机器人支撑，编辑器还需要目标配置目录的写权限。

## 相关说明

- [详细使用说明](../../docs/zh-CN/robotctl.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
