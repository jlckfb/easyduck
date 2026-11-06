# 用手机热点访问机器人

[English](../en/network-access.md) | **简体中文**

开发板保存过某个 Wi-Fi 后，手机热点可以使用相同的名称、密码与兼容的安全模式。开发板连接热点后，手机直接打开：

- 控制页：`http://easyduck.local:9527/`
- 连接设置：`http://easyduck.local/`
- 标定台：通过连接设置或控制页里的切换入口进入。

这是局域网名称，不需要联网查 DNS。手机或热点不支持、屏蔽了 mDNS 时，固定网址可能打不开；此时使用机器人播报的 IP，例如 `http://172.20.10.2:9527/`。不要沿用电脑热点的 `192.168.137.x` 地址。

联网后，地址稳定三秒并经过开机等待期，机器人用中文逐位播报 IP 和控制端口。保持同一地址不重复播报；换网、地址变化或断开较长时间后重新连接会重新播报。策略启用时延后，开始运动会中止正在进行的播报。它只订阅主控状态，不会上力、启动策略或写标定。`[audio] enabled=false` 时不播报。

名称由 Avahi 实际注册结果决定，同一网络有重名设备时可能变为 `easyduck-2.local`；连接设置页展示实际名称。遇到冲突可使用播报 IP。固定网址并不保证所有手机热点都支持，须分别在目标 Android/iPhone 上验收。

## 安装与开机

需要 Debian 的 `avahi-daemon`、`espeak-ng`、`alsa-utils`，以及已有的 NetworkManager、systemd、Python 3.11。语音完全离线，使用 eSpeak NG 的 `cmn-latn-pinyin` 中文拼音声音，数字使用明确声调逐位输入，避免汉字词典回退成英文；语音清晰度和扬声器音量需实机确认。

应用安装器 `deploy/install.py` 包含脚本、名称配置和服务；`deploy/setup/install.sh` 的独立设置页安装也包含此功能。机器人 SDK rootfs 构建入口安装所需软件包，启用 `avahi-daemon.service`、`easyduck-network-access.service`。未更改系统 hostname、默认路由、DNS服务器、DHCP选项或热点密码。

```sh
systemctl status avahi-daemon easyduck-network-access
cat /run/easyduck-network/status.json
```

仅生成测试语音、不播放：

```sh
python3 /opt/robot/network-access/network_access.py --render /tmp/address.wav --address 192.168.43.7
```

应用包不会代替系统包安装。对已有板端安装软件包、更新服务及播放测试音，应先完成部署授权；没有必要重启机器人主控。

## 回退

停止并禁用 `easyduck-network-access.service` 可关闭自动播报。名称发现使用 `avahi-daemon.service.d/50-easyduck.conf` 指向应用自带配置；恢复安装备份中的该覆盖文件（此前不存在则移除），执行 `systemctl daemon-reload` 后重启 Avahi，即可恢复原有名称配置。不要为回退此功能删除 Wi-Fi、蓝牙或舵机/IMU标定文件。

第三方原始许可保留在 Debian 包的 `/usr/share/doc/*/copyright`；来源见仓库 `THIRD_PARTY_NOTICES.md`。

播报停顿：提示语后0.8秒，“地址”后0.6秒，每段IP数字后0.5秒、“点”后0.35秒，端口提示后0.6秒。停顿通过在音频中插入静音实现。
