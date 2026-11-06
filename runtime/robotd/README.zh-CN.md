# robotd — 机器人主控程序

[English](README.md) | **简体中文**

## 职责

正式运行时的硬件控制入口。它接收本机控制请求，管理输入控制权和动作状态，结合反馈运行策略并发布状态。安装配置选择的 XGO 路径以 50 Hz 更新；网页和手柄客户端都不直接写舵机总线。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/main.rs](src/main.rs) | 启动、IPC 请求处理、循环调度与硬件／模型协调 |
| [src/control.rs](src/control.rs) | 控制器状态与策略动作衔接 |
| [src/xgo.rs](src/xgo.rs) | XGO 动作流程与状态切换 |
| [src/control_owner.rs](src/control_owner.rs) | 网页／手柄控制权与回中就绪检查 |
| [src/intents.rs](src/intents.rs) | 共享命令快照与待处理请求 |
| [src/low_battery.rs](src/low_battery.rs) | 低电量抢占、坐下／断力状态与锁定 |
| [src/sound.rs](src/sound.rs) | 音效播放、实时鸣叫与嘴部配合 |
| [src/chorale.rs](src/chorale.rs) | 合唱运行协调 |
| [src/soc.rs](src/soc.rs) | Linux 温度区域读取 |
| [src/params.rs](src/params.rs) | 重新导出 robotd-params 的共享配置定义 |
| [src/main_power_tests.rs](src/main_power_tests.rs) | 主循环上力／断力转换的模拟测试 |

## 调用与数据流

1. `padd`、网页网关和 `robotctl` 经 `/run/robotd.sock` 提交请求；主控校验并记录意图，不把总线交给客户端。
2. 循环将有效意图与 `duck-control` 的传感器快照、模型观测组合，经过动作状态和保护判断后输出舵机目标。
3. 遥测供网页和终端查看；声音结合 `sounds` 合成与主控播放。标定作为独立、互斥的服务运行。

### 工作区直接关系

- 依赖的本地库：[duck-control](../duck-control/README.zh-CN.md)、[duck-ipc-proto](../duck-ipc-proto/README.zh-CN.md)、[kinematics](../kinematics/README.zh-CN.md)、[robotd-params](../robotd-params/README.zh-CN.md)、[sounds](../sounds/README.zh-CN.md)
- 直接使用方：工作区内没有。

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 不要把进程启动、策略加载、舵机上力和开始行走混为一个状态。模板默认关闭策略，实物动作仍需明确操作。
- 通用控制器与 XGO 路径不同。协议中保留的躯干、踢腿等功能不代表当前 XGO 循环执行它们。进程重启也不能证明舵机已断力。

## 相关说明

- [详细使用说明](../../docs/zh-CN/runtime.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
