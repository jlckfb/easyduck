# Linux 服务单元与覆盖配置

[English](README.md) | **简体中文**

## 职责

供 systemd 使用的声明式应用进程管理：启动命令、依赖、权限、重启规则及正式控制／标定互斥。服务单元协调进程，不实现控制算法。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [robotd.service](robotd.service) | 主控进程与机器人 socket 权限 |
| [padd.service](padd.service) | 手柄客户端用户／组与重启设置 |
| [xgoduck-web.service](xgoduck-web.service) | Python 控制网页服务 |
| [xgoduck-calibration.service](xgoduck-calibration.service) | 互斥的标定进程 |
| [easyduck-setup.service](easyduck-setup.service) | 需要登录的连接设置进程 |
| [easyduck-gamepad-reconnect.timer](easyduck-gamepad-reconnect.timer) | 已登记手柄的重连时序 |
| [easyduck-gamepad-reconnect.service](easyduck-gamepad-reconnect.service) | 单次重连命令 |
| [easyduck-network-access.service](easyduck-network-access.service) | 本地地址播报进程 |
| [robotd.service.d/40-xgoduck-reference.conf](robotd.service.d/40-xgoduck-reference.conf) | 选择 XGO 控制路径 |
| [robotd.service.d/50-audio-speaker.conf](robotd.service.d/50-audio-speaker.conf) | 扬声器选择与音效库准备 |
| [padd.service.d/60-control-owner.conf](padd.service.d/60-control-owner.conf) | 受管控制命令参数 |
| [xgoduck-web.service.d/60-control-owner.conf](xgoduck-web.service.d/60-control-owner.conf) | 网页控制服务覆盖配置 |

## 调用与数据流

1. install.py 将单元／覆盖配置放入目标 systemd 目录，start.sh 在明确执行时重新加载并启动正式服务。
2. 设置服务 Navigator 协调交接，systemd Conflicts 避免标定与正式服务同时运行；重启规则针对进程退出，不覆盖所有硬件故障。

## 边界与修改注意

- 安装、启动服务和开机启用是不同操作，要核对合并后的单元与覆盖配置，而不只看基础文件。
- 停止／重启服务可能影响支撑力矩，不能从 inactive 推断实物已断力。部署变更前仍需板端核对。

## 相关说明

- [详细使用说明](../../docs/zh-CN/deployment.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
