# Wi-Fi／蓝牙设置服务

[English](README.md) | **简体中文**

## 职责

提供需要登录的系统连接设置，并协调正式控制与标定交接。它调用 NetworkManager 和 BlueZ，不另建一套机器人控制；动作执行仍由 robotd 负责。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [first_setup.py](first_setup.py) | HTTP／会话认证、设置接口与重连入口 |
| [connections.py](connections.py) | 经过校验的 nmcli／bluetoothctl 后端与连接记录 |
| [navigation.py](navigation.py) | 串行服务交接、断力请求与退出检查 |
| [install.sh](install.sh) | 独立设置安装、备份与服务启用 |
| [static](static) | 登录／设置／导航界面 |
| [../systemd/easyduck-gamepad-reconnect.timer](../systemd/easyduck-gamepad-reconnect.timer) | 已登记手柄的周期重连安排 |

## 调用与数据流

1. 浏览器使用选定的现有系统账号登录，first_setup 将连接请求交给 connections.py。
2. connections.py 调用系统网络／蓝牙工具并核对实际状态，重连定时器重试已信任、已登记的设备。
3. Navigator 结合 systemctl、robotd IPC 和本机标定／控制接口，经确认与状态检查后交接硬件占用。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [static](static/README.zh-CN.md) | 连接设置浏览器资源 |

## 边界与修改注意

- 独立设置安装不创建系统账号、SSH 配置、USB 网络或手柄内核驱动。
- 设置登录不会给独立控制／标定接口增加认证。服务切换可能解除支撑力矩，必须扶稳实物，也不能把请求已接受视为硬件操作必然完成。

## 相关说明

- [详细使用说明](../../docs/zh-CN/connection-setup.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
