# 板级内核集成输入

[English](README.md) | **简体中文**

## 职责

供匹配的外部板级 SDK 使用的输入。UART 半双工与手柄内核支持是两项不同要求，这些文件不会自行构建或安装 Linux 内核。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [uart3-halfduplex-20261021.patch](uart3-halfduplex-20261021.patch) | 对应记录基线的 UART3 半双工补丁 |
| [easyduck-gamepad.config](easyduck-gamepad.config) | 输入／xpad 支持的内核配置片段 |
| [check_gamepad.py](check_gamepad.py) | 只读暂存镜像配置／模块／版本／别名检查 |
| [tests/test_gamepad.py](tests/test_gamepad.py) | 模拟系统命令的检查器回归样例 |
| [../modules-load.d/easyduck-gamepad.conf](../modules-load.d/easyduck-gamepad.conf) | 应用安装的开机加载输入 |

## 调用与数据流

1. SDK 应用匹配的内核输入并编译／打包模块，应用安装器提供模块加载规则和检查器。
2. SDK 在 rootfs／最终镜像检查时调用 check_gamepad.py，读取打包文件与模块索引，不加载驱动或让机器人动作。

## 边界与修改注意

- 模块必须匹配打包内核的版本和 ABI，只有配置片段或串口设备节点并不足够。
- 离线模块／别名检查不是开机或按键验收，保留已测试接收器的范围，不承诺全部 2.4G 设备。

## 相关说明

- [详细使用说明](../../docs/zh-CN/usb-gamepad.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
