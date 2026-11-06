# Access through a phone hotspot

**English** | [简体中文](../zh-CN/network-access.md)

After saving a Wi-Fi profile on the board, a phone hotspot can use the same SSID, password, and compatible security mode. Once connected, open:

- Controller: `http://easyduck.local:9527/`
- Connection setup: `http://easyduck.local/`
- Calibration: enter through the setup/controller navigation.

This is a local network name and does not need Internet DNS. Phones/hotspots that block or lack mDNS may not resolve it. Use the spoken IP instead, for example `http://172.20.10.2:9527/`; do not reuse a PC hotspot's `192.168.137.x` address.

After startup waiting and three seconds with a stable address, the robot announces its IP digit by digit in Chinese, followed by the controller port. It does not repeat an unchanged address; a network/address change or reconnection after a longer disconnect triggers another announcement. Enabled policies defer speech, and motion starting interrupts it. The announcer only subscribes to daemon status: it does not energize servos, enable policies, or write calibration. `[audio] enabled=false` disables speech.

Avahi's actual registration determines the name. A collision can produce `easyduck-2.local`; setup shows the registered name. Use the spoken IP when necessary. Validate target Android/iPhone hotspots individually; a fixed hostname does not guarantee support on every hotspot.

## Installation and boot

Requires Debian `avahi-daemon`, `espeak-ng`, and `alsa-utils`, plus existing NetworkManager, systemd, and Python 3.11. Speech is offline, using eSpeak NG's `cmn-latn-pinyin` voice and explicit tones per digit to avoid dictionary fallback to English. Check intelligibility and speaker volume on hardware.

`deploy/install.py` installs scripts, name configuration, and service units; standalone `deploy/setup/install.sh` also includes this feature. The robot SDK rootfs build installs packages and enables `avahi-daemon.service` and `easyduck-network-access.service`. This feature does not change the system hostname, default route, DNS server, DHCP options, or hotspot password.

```sh
systemctl status avahi-daemon easyduck-network-access
cat /run/easyduck-network/status.json
```

Render test speech without playback:

```sh
python3 /opt/robot/network-access/network_access.py --render /tmp/address.wav --address 192.168.43.7
```

Application installation does not replace system-package installation. Installing packages, updating services, or playing test audio on a board requires deployment authorization; restarting the robot daemon is unnecessary for this feature.

## Rollback

Stop and disable `easyduck-network-access.service` to disable automatic speech. Name discovery uses `avahi-daemon.service.d/50-easyduck.conf` to select application configuration. Restore the override from the installation backup (remove it if originally absent), run `systemctl daemon-reload`, and restart Avahi to restore earlier naming. Do not delete Wi-Fi, Bluetooth, or servo/IMU calibration for this rollback.

Original third-party licenses remain in Debian packages' `/usr/share/doc/*/copyright`; see repository [third-party notices](../../THIRD_PARTY_NOTICES.md).

Speech includes 0.8 seconds of silence after the opening prompt, 0.6 after “address,” 0.5 after each address segment's digits, 0.35 after “dot,” and 0.6 after the port prompt. Silence is inserted into the audio itself.
