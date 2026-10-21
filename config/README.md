# 配置

- `examples/`：新机器的格式样例，策略默认关闭，零点 2048 只是示例值，不是可直接运行的标定。使用前设置零点、方向、限位和 IMU 安装信息。
- `board/`：特定机器的参考参数，不是通用标定，也不代表其他实物的可用姿态。

## 主控参数

`robotd.toml` 设置串口、控制频率、策略文件、IMU 和音效。参考配置使用 `/dev/ttyS3`、50Hz，IMU 为 `/dev/i2c-2` 地址 `0x6B`，采用 SFLP。

基础增益为 P=5、D=20；`scs_joint_gains.json` 将 30、31、14、24 号的 P 覆盖为 10。增益应结合自己的机械结构和负载检查。

## 标定文件

- `visual_calibration.json`：零点、方向、限位及 IMU 安装与偏置，驱动优先读取。
- `zero_pos.json`：标定台的基线输入。主控在 visual 文件不存在时从这里读取限位，并要求零点、方向与驱动内置基线一致；不能仅修改此文件来更换主控零点。
- `calibration_speed_restore.json`：标定台保存的舵机速度恢复值。

新机器应使用标定台生成匹配的 `visual_calibration.json`，不要直接套用 `board/` 的标定结果。

运行时配置放在 `/etc/robot/`，主控在启动时读取。安装器默认保留已有配置，显式指定 `--replace-config` 才替换并备份。

## 音效

`voice.env` 设置生成种子；默认值为 `3239431280`。服务启动前由 `sounds` 生成音效库，不需要在源码中保存 WAV 文件。

## 文件选择与安装

安装器使用 `--profile examples` 或 `--profile board` 选择整套配置。`examples/robotd.toml` 的 `policy.enabled` 为 `false`，`board/robotd.toml` 为 `true`；关闭策略不代表所有控制命令都不会上力。启动和动作前需核对标定及机器人状态。

不要把 Wi-Fi 密码、SSH 密钥或令牌写进这些参考配置；个人私有配置可放在 Git 忽略的 `config/private/`。安装步骤见[编译与部署](../deploy/README.md)。
