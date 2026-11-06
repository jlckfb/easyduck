# 本地名称与 IP 播报

[English](README.md) | **简体中文**

## 职责

提供离线地址播报和应用 Avahi 名称配置，观察网络／机器人状态，在策略活动时延后语音；不负责配网，也不控制舵机。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [network_access.py](network_access.py) | 地址稳定判断、状态订阅、离线语音生成／播放与状态文件 |
| [avahi-daemon.conf](avahi-daemon.conf) | 应用 mDNS 名称配置 |
| [../systemd/easyduck-network-access.service](../systemd/easyduck-network-access.service) | 启动／重启与文件系统权限 |
| [../systemd/avahi-daemon.service.d/50-easyduck.conf](../systemd/avahi-daemon.service.d/50-easyduck.conf) | 选择应用名称配置的 Avahi 覆盖文件 |

## 调用与数据流

1. 系统网络提供地址，Avahi 提供实际注册的局域网名称。
2. Python 服务订阅 robotd 状态，调用 eSpeak NG／ALSA 播报，并在 /run/easyduck-network 写入状态。

## 边界与修改注意

- mDNS 是否可用取决于网络和客户端；播报 IP 是回退方式，不是跨热点固定地址保证。
- 不能为修播报而修改路由、DNS、热点凭据或机器人标定；语音文件生成成功不代表扬声器已播放。

## 相关说明

- [详细使用说明](../../docs/zh-CN/network-access.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
