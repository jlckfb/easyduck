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

## 新机器人：分配舵机 ID

从连接设置页点击“新机器人：分配舵机 ID”，或在标定台选择“舵机 ID 分配”页签。新机入口先停止正式控制和手柄服务，不要求15个既定ID在线，也不会自动上力。返回连接设置页时，标定服务继续独占串口，正式控制保持停止。

1. 已编号且 ID 不重复的舵机可保留连接，每次只新增一只出厂 ID 为 1 的舵机，点击“扫描 ID 0～253”。可中途停止并保留已发现的地址。当前总线速率为1Mbps；其他速率不在此页面的设置范围内。
2. 选择扫描结果，再按安装位置选择目标关节。右腿10～14、左腿20～24、颈头嘴30～34。页面显示原始位置和力矩状态。
3. 如果仍上力，先点“所选舵机松力矩”。确认只有一只舵机使用所选 ID，勾选确认后，点击“写入目标 ID 并核对”。新ID必须未被占用；254和255为保留地址。
4. 成功后会确认新地址响应、旧地址不再响应、舵机断力及存储锁恢复。已编号舵机可保持连接，再新增下一只 ID 为 1 的舵机，重新扫描后继续。不要同时接入两只相同 ID 的舵机，扫描无法可靠区分它们。
5. 分配完成后接齐舵机，停止扫描，切回姿态标定，重新核对HOME、方向、限位和IMU。不要直接使用其他机器人的标定运行策略。

两只相同ID的舵机无法通过扫描可靠区分，必须逐只接线。校验错误或舵机报警会停止扫描，不把它们当成“未发现”。改号不写P/D、限流、位置目标或中位，进入页面和扫描也不写参数。改号前后记录默认保存到 `/var/lib/easyduck/servo-id-changes.jsonl`；可用 `--id-journal` 指定路径。若写入或回读中断，页面不会报告成功，须重新扫描核对实际ID。需要改回旧ID时按同样流程单独改号。

交互流程参考 [XGO Duck 官方首次舵机设置](https://github.com/LuwuDynamics/xgoduck_runtime_arduino#first-time-servo-setup)，使用本项目的泰山派串口后台实现。

仅在PC模拟单只出厂ID为1的舵机：

```sh
python3 calibration/server.py --mock --mock-servo-id 1 --bind 127.0.0.1 --http-port 18766 --baseline config/examples/zero_pos.json --profile /tmp/easyduck-id-preview.json --dry-run-apply
```

模拟操作不访问真实串口。`--mock-servo-id` 必须与 `--mock` 一起使用。
