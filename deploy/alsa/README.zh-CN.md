# 板端声卡状态

[English](README.md) | **简体中文**

## 职责

应用安装时使用的目标声卡 ALSA 混音状态，配合运行时音频设备配置和启动前扬声器选择命令使用，不是音效合成模块。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [asound.state](asound.state) | 目标混音器／控制项状态 |
| [../systemd/robotd.service.d/50-audio-speaker.conf](../systemd/robotd.service.d/50-audio-speaker.conf) | 扬声器选择与音效库准备 |
| [../../runtime/sounds/src/main.rs](../../runtime/sounds/src/main.rs) | 生成／播放声音的命令入口 |

## 调用与数据流

1. install.py 将 asound.state 放入 var/lib/alsa，系统 ALSA 恢复与服务覆盖配置准备输出路由。
2. robotd／sounds 按运行配置使用设备与音效库，本文件没有机器人动作逻辑。

## 边界与修改注意

- 混音控制项与板卡／声卡相关，不能假定该快照适用于其他设备。
- 文件安装或声音导出不能证明实际出声；播放变更与动作是不同操作，也需相应授权。

## 相关说明

- [详细使用说明](../../docs/zh-CN/sounds.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
