# 应用访问用系统身份

[English](README.md) | **简体中文**

## 职责

systemd-sysusers 声明，提供机器人 socket 访问组和专用手柄用户；它们建立应用访问身份，不创建网页登录账号或默认密码。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [robot.conf](robot.conf) | 创建本机 IPC 访问用 robot 组 |
| [padd.conf](padd.conf) | 创建专用 padd 用户／组 |
| [../start.sh](../start.sh) | 正式启动前调用 systemd-sysusers |
| [../systemd/padd.service](../systemd/padd.service) | 给 padd 赋予 input／robot 附加组 |

## 调用与数据流

1. install.py 将声明复制到 usr/lib/sysusers.d，start.sh 明确调用 systemd-sysusers。
2. 服务单元的用户／组设置再决定 Linux 输入设备和 robotd socket 的访问权限。

## 边界与修改注意

- robot.conf 声明的是组，不是 robot 登录用户；账号和密码设置要分开处理。
- 不能通过全面开放硬件／socket 权限来掩盖权限问题，应核对目标身份与合并后的服务权限。

## 相关说明

- [详细使用说明](../../docs/zh-CN/deployment.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
