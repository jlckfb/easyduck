# sounds — 可重复音效合成

[English](README.md) | **简体中文**

## 职责

同时提供音效生成命令和被 robotd 链接的合成库。种子决定音色，标签／变体选择配方。生成音效库会输出 48 kHz 单声道 WAV 与版本／种子信息，本身不驱动机器人。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/main.rs](src/main.rs) | ensure-bank／render／play 命令入口 |
| [src/lib.rs](src/lib.rs) | 音效库版本、标签、配方与生成 |
| [src/voices.rs](src/voices.rs) | 包含低电量提示的声音配方 |
| [src/synth.rs](src/synth.rs) | 合成基础与采样率 |
| [src/personality.rs](src/personality.rs) | 由种子得到的音色特征 |
| [src/rng.rs](src/rng.rs) | 确定性随机生成 |
| [src/stream.rs](src/stream.rs) | 流式声音生成 |
| [src/chorale](src/chorale) | 曲谱解析、节拍、声部分配与渲染 |
| [scores](scores) | 文本与 MIDI 曲谱输入 |

## 调用与数据流

1. 音频 systemd 覆盖配置在主控启动前执行 sounds ensure-bank，使用已配置的音色种子。
2. robotd 的声音模块协调播放、鸣叫与嘴部命令；ALSA 和设备配置仍由系统提供。
3. 曲谱／合成辅助支持合唱路径，但仓库没有提供完整的多机器人发现与传输服务。

### 工作区直接关系

- 依赖的本地库：工作区内没有。
- 直接使用方：[robotd](../robotd/README.zh-CN.md)

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 生成的 WAV 不放入源码仓库，种子和版本匹配时可复用音效库；配方变化可能需要调整库版本。
- 保留曲谱与配方来源。文件导出成功不代表扬声器能播放，生成音效库也不会自动启用合唱或动作。

## 相关说明

- [详细使用说明](../../docs/zh-CN/sounds.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
