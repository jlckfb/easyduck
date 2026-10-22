# 舵机与 IMU 标定台

Python 3.9+ 标准库服务，默认端口 8766。页面使用本地 three.js 显示机器人模型，提供舵机零点、方向、限位与 IMU 标定。

默认串口 `/dev/ttyS3`，基线 `/etc/robot/zero_pos.json`，保存配置 `/etc/robot/visual_calibration.json`。正式标定服务与 `robotd`、`padd`、`xgoduck-web` 互斥；同一时刻只能有一个程序占用舵机串口。标定操作可能上力、改变姿态或写入配置，操作前承托机器人。

## 无实物预览

在本目录执行：

```sh
mkdir -p ../build
python3 server.py --mock --bind 127.0.0.1 --http-port 8766 --baseline ../config/examples/zero_pos.json --profile ../build/mock-profile.json --dry-run-apply
```

此命令使用模拟总线，访问 `http://127.0.0.1:8766`。`--dry-run-apply` 只显示服务切换命令。

`imu/` 包含 GPL-3.0-or-later 的 IMU 读取模块；显示素材与浏览器依赖的授权见[第三方声明](../THIRD_PARTY_NOTICES.md)。

速度恢复记录位于 `/etc/robot/calibration_speed_restore.json`，用于标定退出后恢复原速度参数。

## 切换页面

可从连接设置页或机器人控制页进入标定台。导航由 [连接设置服务](../deploy/setup/README.md) 协调，三个页面沿用同一无线主机地址；设置页端口通过 `/etc/easyduck/setup.env` 配置。

标定页顶部可返回机器人控制或连接设置。切换前须保存或撤销草稿、停止实物运动、全部松力矩；未完成时会保留当前页面并提示。退出时冻结新的标定操作，关闭串口并恢复临时速度后才恢复正式服务。切换不会自动保存标定、上力或运行策略。
