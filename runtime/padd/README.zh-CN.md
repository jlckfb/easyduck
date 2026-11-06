# padd — 手柄输入客户端

[English](README.md) | **简体中文**

## 职责

读取统一后的摇杆和按键，并转换成机器人控制意图。Linux 输入驱动与 gilrs 处理设备表示；padd 负责操作模式、按键动作、命令时序与 IPC，另提供用于诊断的只读输入流。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [Cargo.toml](Cargo.toml) | gilrs、IPC 及 Linux evdev／libudev 相关依赖 |
| [src/main.rs](src/main.rs) | 设备事件、键位映射、行走／头部模式、速度处理和受管控制消息 |
| [src/tap.rs](src/tap.rs) | Linux 原始事件观察与有界订阅队列 |
| [../../deploy/systemd/padd.service](../../deploy/systemd/padd.service) | 运行用户、输入／socket 权限和重启规则 |
| [../../deploy/systemd/padd.service.d/60-control-owner.conf](../../deploy/systemd/padd.service.d/60-control-owner.conf) | 安装使用的受管控制及命令范围参数 |

## 调用与数据流

1. USB／2.4G 接收器依赖对应内核驱动，蓝牙手柄依赖系统蓝牙与输入栈，两者最终呈现为 Linux 输入设备。
2. gilrs 事件进入 `main.rs` 的映射和模式逻辑，请求交给 robotd，由主控决定控制权与执行。
3. `robotctl monitor` 可订阅输入流用于排查，输入流本身不占用或控制舵机。

### 工作区直接关系

- 依赖的本地库：[duck-ipc-proto](../duck-ipc-proto/README.zh-CN.md)
- 直接使用方：工作区内没有。

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 配对和加载 xpad 不由本程序完成；已有映射不代表兼容所有品牌手柄。
- Start／行走、捡物、紧急断力和长按关机是不同动作。控制权切换不能描述成自动开始运动，当前工作目录的修改也不代表已部署程序。

## 相关说明

- [详细使用说明](../../docs/zh-CN/usb-gamepad.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
