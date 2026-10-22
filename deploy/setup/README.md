# 首次连接设置

适用于保留泰山派系统管理页的 R3/R4 基础系统，需要系统已有 `/usr/local/sbin/tspi-web.py`、`tspi-web.service`、NetworkManager、BlueZ 和 SSH 服务。扩展复用系统页的登录验证，不包含或设置登录密码。

## 使用

1. 首次烧录后，用 USB 连接电脑，打开 `http://192.168.7.1/`，使用系统管理页账号登录。
2. 点击“扫描 Wi-Fi”，选择或输入热点名称，填写密码并连接。连接成功后会保存自动重连设置，并显示无线 IP。
3. 将蓝牙手柄置于配对模式，点击“扫描蓝牙设备”，选择自己的手柄并配对。配对会保存设备信任状态，不发送机器人动作。
4. 电脑与机器人使用同一路由器或电脑热点。点击“通过 Wi-Fi 控制机器人”，确认无线控制页可打开后拔掉 USB。日常控制地址为 `http://<无线IP>:9527/`，管理页为 `http://<无线IP>/`；SSH 使用 `ssh <系统用户名>@<无线IP>`，不需要建立网页转发隧道。

无线地址由热点或路由器分配，可能改变；需要固定地址时可在路由器配置 DHCP 地址保留。Wi-Fi 断开后，USB 地址 `192.168.7.1` 仍可用于重新设置。蓝牙配对失败时请确认设备未连接其他主机，并重新进入配对模式。

## 安装及回退

`deploy/install.py` 将程序安装到 `/opt/robot/setup/first_setup.py`，并安装 `tspi-web.service.d/60-easyduck-setup.conf`。在上述基础系统中执行 `systemctl daemon-reload` 和 `systemctl restart tspi-web` 即可启用。服务同时接受 USB 和 Wi-Fi 连接，系统原管理功能通过“系统设置”进入。

本程序只管理网络和蓝牙，既不访问舵机总线，也不发送机器人控制指令。机器人控制仍由正式控制网页和 `robotd` 提供。

回退可使用安装器备份；也可移除本扩展的 systemd 覆盖文件，再执行 `systemctl daemon-reload`、`systemctl restart tspi-web`，恢复原 USB 系统管理页。回退网页不会删除已保存的 Wi-Fi 或蓝牙配对。
