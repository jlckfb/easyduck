# robotctl 命令行工具

通过 Unix socket 调用机器人服务，查询状态、显示实时数据和执行控制命令；也提供本地参数编辑器。舵机总线由 `robotd` 管理，`robotctl` 不直接打开舵机串口或 IMU。

## 构建

在仓库的 `runtime/` 目录执行：

```sh
cargo build --locked --release -p robotctl
```

依赖工作区中的 `duck-ipc-proto`（通信协议）、`robotd-params`（参数定义及校验）和 `kinematics`（几何计算），不能只复制本目录单独构建。运行时需要 Linux/Unix socket；终端界面需要交互终端。

## 查询与监控

以下命令查询状态，不请求上力或运动：

```sh
robotctl --help
robotctl --version
robotctl robot mode --json
robotctl monitor
robotctl monitor --hz 10 --json
```

主控 socket 默认为 `/run/robotd.sock`，可通过 `--robot-socket` 指定其他路径。`monitor --json` 每行输出一个 JSON 对象，可重定向保存；`Ctrl-C` 退出。

监控页显示主控状态、策略、关节目标和反馈、IMU、供电信息及终端 3D 姿态。手柄原始事件需要 `padd` 的事件流；深度画面需要另行提供 ToF 服务。

| 监控按键 | 用途 |
| --- | --- |
| `q` / `Esc` / `Ctrl-C` | 退出 |
| 上下方向键、`j` / `k` | 滚动关节列表 |
| `PageUp` / `PageDown` / `Home` | 翻页或回到开头 |
| `u` | 切换角度单位 |
| `p` / `t` / `d` | 切换手柄、深度、3D 显示 |
| 左右方向键、`[` / `]` | 旋转 3D 视角 |

`assets/duck.bin` 是编译时嵌入的终端显示网格，不是动作模型；运行时无需另行复制它。3D 图反映反馈数据，不代替实物方向核对。

## 参数编辑

```sh
sudo robotctl configure --file /etc/robot/robotd.toml
```

上下键选择，空格切换开关，回车编辑，`u` 恢复该项默认值，`q` 进入退出流程。保存前按主控的参数定义校验；保留未修改的注释、顺序和未知字段，并原子替换配置文件。需要目标目录的写权限。

保存后界面提供相关服务的重启选项。重启会影响机器人控制，需先把机器人放稳。编辑可选服务的参数不代表已安装该服务。

## 控制命令与服务范围

使用 `robotctl <命令> --help` 查看具体参数。

| 命令 | 作用与条件 |
| --- | --- |
| `robot init` | 请求上力并回到初始姿态，会运动 |
| `robot relax --yes` | 请求断力，未支撑的机器人会失去支撑 |
| `robot do <skill>` | 请求动作，依赖主控状态及对应动作模型 |
| `robot look <x> <y> <z>` | 请求头部转向；坐标为躯干前、左、上，单位米 |
| `quack` | 请求主控播放叫声 |
| `chorale` | 请求合唱，需启用配置；可能带动头部和嘴部 |
| `theremin` | 请求体感乐器，需深度数据服务和对应配置 |
| `health` / `version` | 汇总多个服务状态；缺失的服务会显示不可达，不能据此单独认定舵机故障 |
| `net` / `system` / `pad` | 保留的客户端接口，需要额外的 `configd`；本软件库未提供该服务 |
| `update` | 保留的更新客户端，需要额外的 `updaterd`；本软件库未提供该服务 |
| `completions <shell>` | 输出当前命令的 shell 补全脚本 |

手柄实际控制由 `padd` 完成；保留的 `pad pair` 客户端不能代替本软件库之外的配对服务。带标记的旧蓝牙兼容流程还可能暂停 `btd` 和重置适配器。

## 返回值

`0` 成功，`1` 执行失败，`2` 参数错误，`3` 服务不可达，`4` 更新忙，`5` 请求被拒绝，`6` 权限不足。解析参数错误时的帮助信息以当前程序输出为准。

## 测试

```sh
cargo test --locked -p robotctl
```

源码内单元测试覆盖命令解析、模拟通信、参数保存和显示逻辑。这些测试无需实物连接，也不能证明实物动作正确。
