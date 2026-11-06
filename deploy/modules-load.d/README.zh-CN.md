# 手柄开机加载规则

[English](README.md) | **简体中文**

## 职责

提供开机请求加载 xpad 的 systemd modules-load 配置，只声明模块名称，不包含或编译内核驱动。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [easyduck-gamepad.conf](easyduck-gamepad.conf) | 为兼容 Xbox／XInput 接收器请求 xpad |
| [../kernel/easyduck-gamepad.config](../kernel/easyduck-gamepad.config) | SDK 内核配置输入 |
| [../kernel/check_gamepad.py](../kernel/check_gamepad.py) | 检查暂存镜像的规则／模块／版本／别名 |

## 调用与数据流

1. 应用安装器将规则放入 etc/modules-load.d。
2. 开机加载器使用 SDK 已打包的模块，随后 Linux 输入设备向 padd 提供事件。

## 边界与修改注意

- 只有规则而没有匹配 xpad 模块，不能增加支持；模块版本／ABI 与 depmod 索引必须匹配镜像。
- 模块加载成功不等于接收器已连接、键位正确或机器人能动作。

## 相关说明

- [详细使用说明](../../docs/zh-CN/usb-gamepad.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
