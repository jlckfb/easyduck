# 机器人音效

[English](../en/sounds.md) | **简体中文**

`sounds` 按固定种子生成48kHz单声道WAV，供主控播放启动音、鸭叫、提示与告警。种子决定音色，变体用于同类声音的变化。

## 音效库生成

```sh
sounds ensure-bank --dir /var/lib/robot/sounds --seed 3239431280
```

已有匹配种子和版本的音效库会复用。板端由主控服务的音频启动配置调用该命令，种子可在 `/etc/robot/voice.env` 中设置。生成的WAV不放进源码仓库。

## 离线导出

```sh
sounds render chirp chirp.wav --seed 3239431280 --variant 0
sounds render-all output-sounds --seed 3239431280
```

这两条命令只写音频文件。输出文件应放在工作目录以外或被忽略的构建目录。

## 播放

泰山派RK809声卡使用ALSA工具：

```sh
sounds play chirp --seed 3239431280 --device plughw:0,0
```

主控通过 `robotd.toml` 的 `[audio]` 指定设备和音效库路径。`deploy/alsa/asound.state` 提供声卡配置，服务启动前选择扬声器输出。

## 音乐与实时合成

- `runtime/sounds/scores/wistful.duckscore`：默认文本曲谱，同时说明文本格式。
- `runtime/sounds/scores/duck_strut.mid`：内置MIDI曲谱。
- `runtime/sounds/src/chorale/`：曲谱解析、声部分配、节拍跟踪和渲染。
- `robotd` 中的音频模块对接播放、合唱和实时音频参数；后两项需要对应输入及配置，不随音频库生成自动启用。

音效配方改编自 `apirrone/microduck_sounds`，Rust实现随本项目采用其声明的许可证；曲谱及第三方素材仍需分别适用各自授权。
