# Wi-Fi 与 PS5 手柄连接设置

[English](../en/connection-setup.md) | **简体中文**

本地配置网页，使用 NetworkManager 管理 Wi-Fi、BlueZ 管理 DualSense 手柄。Python 标准库、原生 HTML/CSS/JavaScript 实现，无前端框架和外部 CDN，不包含显示屏、声卡、蜂窝网络等板卡专用逻辑。

## 系统要求

- Linux、systemd、Python 3.9 或更高版本。
- NetworkManager / `nmcli`、BlueZ / `bluetoothctl`，以及系统自带的 `libcrypt`。
- 系统已识别 Wi-Fi 和蓝牙硬件；不额外安装手柄驱动。
- 使用设备上已有的系统账号密码登录。程序不创建账号、不设置默认密码、不更改 SSH 登录配置。

Debian/Ubuntu 可安装以下系统包：

```sh
sudo apt install python3 network-manager bluez libcrypt1
```

## 安装与启动

在仓库根目录执行，仅安装配置网页及手柄重连服务：

```sh
sudo bash deploy/setup/install.sh --port 80 --user <系统用户名>
```

网页默认登录账号为 `lckfb`；系统账号由机器人 SDK 的 rootfs 安装入口创建。已有 `/etc/easyduck/setup.env` 中的显式账号设置优先。安装器保留已有 `/etc/easyduck/setup.env`，备份覆盖的文件并输出回退清单，然后启动网页和重连定时器。如果端口已被其他网页占用，请先协调原服务，或选择其他端口，例如 `--port 8081`。

```sh
systemctl status easyduck-setup.service
systemctl status easyduck-gamepad-reconnect.timer
journalctl -u easyduck-setup.service
```

安装后通过 `http://<设备可达IP>:<端口>/` 打开页面。程序默认监听全部 IPv4 网卡，不指定网卡名或固定 IP。USB 网卡、Wi-Fi、以太网的地址由系统配置，此网页不创建 USB 网络或热点。

程序位于 `/opt/robot/setup/`，配置位于 `/etc/easyduck/setup.env`：

| 配置 | 用途 |
| --- | --- |
| `EASYDUCK_SETUP_USER` | 允许登录的系统账号 |
| `EASYDUCK_SETUP_HOST` | 监听地址，默认 `0.0.0.0` |
| `EASYDUCK_SETUP_PORT` | 网页端口，默认 `80` |
| `EASYDUCK_CONTROL_PORT` | 已有机器人控制页端口，默认 `9527` |
| `EASYDUCK_SETUP_STATE` | 手柄重连信息目录，默认 `/var/lib/easyduck/setup` |

修改配置后执行 `sudo systemctl restart easyduck-setup.service`。网页服务需要读取系统账号密码哈希，并管理网络配置，因此由 root 运行；密码不会写入网页日志或命令行参数。

## Wi-Fi 配网

1. 点击“扫描网络”，列表显示 SSID、信号强度和加密方式。
2. 列表中的“连接”仅选中网络：填写下方 SSID，并将焦点移到密码框，不会立即连接。
3. 输入密码，点击下方“连接 Wi-Fi”。开放网络留空；支持个人 WPA2/WPA3，企业认证和 WEP 不在本页面范围内。
4. 右下角显示连接进度及结果。错误提示可选中复制，并保留到点击关闭。只有目标连接 UUID、SSID、激活状态和 IP 全部匹配才报告成功，仍连接旧网络不会被视为成功。
5. 成功后自动保存配置并启用开机连接。左侧显示无线控制网页、设置网页和 SSH 地址。

密码错误、网络未找到、认证超时、已关联但未取得 IP（DHCP 无响应）分别提示。密码保存在 NetworkManager 的 root 专用连接文件中；失败时清理本次创建的配置，不删除其他网络配置。

切换 Wi-Fi 可能使当前浏览器网络中断。可先使用 USB 或其他稳定连接配网，再从新无线地址打开网页。电脑和设备需处于可互访的局域网；SSH 与控制网页可以独立使用，无需网页转发隧道。

## 页面切换

连接设置、机器人控制、实物标定台均提供相互切换入口。Wi-Fi 连接成功后，设置页的控制页和标定页入口优先使用无线 IP。其他页面继续使用该无线主机名，不依赖 USB 地址。

标定台与正式控制共用舵机串口，必须互斥运行。切换由登录后的设置服务协调：

- 进入标定前先放稳或托住机器人，在切换页确认断力。服务停止手柄控制、请求断力、停止正式控制后才启动标定台，并逐个回读确认全部断力；通信或供电故障可能导致无法确认。
- 标定页有未保存草稿、实物运动任务或未确认断力时，不能切换到正式控制或设置页。先保存或撤销草稿、停止运动、全部松力矩。
- 标定退出时冻结新的操作、释放串口并恢复临时速度，再启动正式控制和手柄服务。退出异常时保留提示，不强行恢复控制。
- 离开控制页会释放该页面的控制连接。切换不会自动上力、保存标定或启动策略；进入正式控制后仍需用户自行选择动作。

设置页端口由 `EASYDUCK_SETUP_PORT` 配置，标定页端口由 `EASYDUCK_CALIBRATION_PORT` 配置（默认 `8766`），控制页使用 `EASYDUCK_CONTROL_PORT`。各页面服务通过 `/etc/easyduck/setup.env` 读取设置页端口；如果改变控制页或标定页监听端口，应同时修改其服务启动参数。

## PS5 手柄

1. 同时按住 **PS 键和 Create 键**，直到灯条快速闪烁。
2. 点击“扫描手柄”，选择 `Wireless Controller` 或 `DualSense Wireless Controller`。
3. 点击“配对并连接”。服务依次执行 `pair`、`trust`、`connect`，并核对实际状态。
4. 手柄休眠或关机时，先按一下 PS 键唤醒。已配对设备可单独连接、断开或忘记。设备名、MAC、配对和连接状态、电量、输入设备路径会局部更新。

电量优先读取 BlueZ，随后查找与该 MAC 对应的系统电源设备；系统不提供电量时隐藏电量信息。输入设备仅通过系统元数据识别 `/dev/input/js*`、`event*`，不打开输入事件流、不调用独占接口、不影响已有手柄控制服务。

成功信任并登记的设备由 systemd 定时器在开机后重试连接。手动断开会暂停本次开机期间的自动重连，防止刚断开就被连回；点击连接或重启设备后恢复。忘记设备会删除登记和系统配对。

没有适配器、手柄未进入配对模式、认证失败、连接被占用和配对超时会分别提示。系统不能可靠判断手柄是否连接了另一台主机时，页面会给出排查提示，不把推测显示为已确认的原因。

## 验收步骤

测试连接操作前应停止实物动作并放稳机器人，避免现有手柄服务在按键时触发动作。本网页不会向机器人发送运动指令。

| 项目 | 操作与预期 |
| --- | --- |
| Wi-Fi 正确密码 | 选择目标网络并连接；目标 SSID、网卡和 IP 正确，左侧出现无线地址 |
| Wi-Fi 错误密码 | 使用有效长度的错误密码；提示密码或认证错误，不把旧网络显示为目标连接成功 |
| DHCP 无响应 | 在测试热点关闭 DHCP；关联后提示未取得 IP，不报告成功 |
| 网络未找到 | 输入当前不可见的 SSID；提示未找到网络 |
| PS5 配对 | PS + Create 进入配对模式，扫描并配对；设备显示已配对、已连接 |
| 断开与连接 | 点击断开后保持断开；点击连接后恢复，已有手柄服务仍可使用输入设备 |
| 自动重连 | 登记后关闭手柄再开启，或重启设备；定时器重试后恢复连接 |
| 忘记设备 | 点击忘记，配对和重连登记被清理；重新使用需配对 |
| 电量 | 系统支持时显示百分比，读不到时隐藏电量信息 |

界面检查清单：

- [ ] 暗／亮主题可切换，刷新后保留选择。
- [ ] 扫描、配网、配对、连接、断开、忘记操作不整页刷新。
- [ ] 进度和结果只显示在右下角 toast，不出现重复的正文操作提示。
- [ ] 后台状态刷新不清空正在填写的 SSID 或密码。
- [ ] 手机宽度下没有横向溢出，卡片和按钮均可操作。
- [ ] 未登录或校验失败的请求不能执行连接操作。

## 回退

先停止本次安装的服务，再使用安装器输出的备份目录执行仓库的回退工具：

```sh
sudo systemctl disable --now easyduck-setup.service easyduck-gamepad-reconnect.timer
sudo systemctl stop easyduck-gamepad-reconnect.service
sudo python3 deploy/rollback.py --root / --backup <备份目录> --restore-on-board
sudo systemctl daemon-reload
```

网页程序回退不会删除已保存的 Wi-Fi、蓝牙配对和重连登记。需要删除配对时可先通过页面“忘记设备”。如果安装时调整过其他网页服务的端口，回退后应按其原配置恢复。
