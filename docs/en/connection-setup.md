# Wi-Fi and PS5 connection setup

**English** | [简体中文](../zh-CN/connection-setup.md)

The local setup page uses NetworkManager for Wi-Fi and BlueZ for DualSense pairing. It uses Python's standard library and native HTML/CSS/JavaScript, without a frontend framework or external CDN. Display, sound-card, cellular, and other board-specific setup are outside its scope.

## Requirements

- Linux, systemd, Python 3.9+ for this service.
- NetworkManager / `nmcli`, BlueZ / `bluetoothctl`, and system `libcrypt`.
- Wi-Fi/Bluetooth hardware already recognized by Linux; this installer does not add gamepad drivers.
- An existing system account/password for login. The program does not create accounts, set default passwords, or change SSH login configuration.

On Debian/Ubuntu:

```sh
sudo apt install python3 network-manager bluez libcrypt1
```

## Installation and startup

From the repository root, install only setup and gamepad reconnection services:

```sh
sudo bash deploy/setup/install.sh --port 80 --user <SYSTEM_USER>
```

The default web-login account is `lckfb`, created by the robot SDK rootfs integration. An explicit account in an existing `/etc/easyduck/setup.env` takes precedence. The installer preserves this file, backs up replacements, prints a rollback manifest, and starts setup plus the reconnection timer. Coordinate an existing listener before reusing its port, or select another port such as `--port 8081`.

```sh
systemctl status easyduck-setup.service
systemctl status easyduck-gamepad-reconnect.timer
journalctl -u easyduck-setup.service
```

Open `http://BOARD_IP:PORT/`. Setup listens on all IPv4 interfaces by default, without choosing an interface or fixed IP. The system manages USB/Wi-Fi/Ethernet addresses; this page creates neither USB networking nor a hotspot.

Programs install under `/opt/robot/setup/`; settings are in `/etc/easyduck/setup.env`:

| Setting | Purpose |
| --- | --- |
| `EASYDUCK_SETUP_USER` | System account allowed to log in |
| `EASYDUCK_SETUP_HOST` | Bind address, default `0.0.0.0` |
| `EASYDUCK_SETUP_PORT` | Setup port, default `80` |
| `EASYDUCK_CONTROL_PORT` | Controller page port, default `9527` |
| `EASYDUCK_SETUP_STATE` | Gamepad reconnection state, default `/var/lib/easyduck/setup` |

After edits, run `sudo systemctl restart easyduck-setup.service`. The service runs as root to read system password hashes and manage network settings. Passwords are not written to web logs or command arguments.

## Wi-Fi

1. Choose **扫描网络** (scan networks) to list SSID, signal, and security.
2. A row's connect button selects the SSID and focuses the password field; it does not connect yet.
3. Enter the password and choose **连接 Wi-Fi**. Leave it empty for open networks. Personal WPA2/WPA3 are supported; enterprise authentication and WEP are not.
4. Progress/results appear at the bottom right. Errors can be selected/copied and remain until dismissed. Success requires the target UUID, SSID, active state, and IP all to match; an old connection does not count.
5. Success saves the profile and enables automatic connection. The left side shows wireless controller, setup, and SSH addresses.

Wrong password, missing network, authentication timeout, and association without DHCP are distinct results. NetworkManager stores credentials in a root-only connection file. Failure removes this attempt's new profile, not other networks.

Changing Wi-Fi may interrupt the browser connection. Configure through USB or another stable link, then use the wireless address. Device and computer must be mutually reachable on the LAN; SSH and browser control work independently without a web forwarding tunnel.

## Page handoff

Setup, control, and calibration offer cross-navigation. After Wi-Fi connects, setup prefers the wireless IP for control/calibration; subsequent pages retain that wireless host instead of relying on USB.

Calibration and normal control share the serial bus and must be exclusive. Authenticated setup navigation coordinates switching:

- Before calibration, support the robot and confirm torque release. Stop gamepad input, request release, stop normal control, start calibration, then read each servo to check release. A power/communication fault can prevent confirmation.
- Unsaved drafts, physical motion, or unconfirmed torque release block leaving calibration. Save/discard drafts, stop motion, and release torque first.
- Exit freezes operations, releases serial, and restores temporary speed settings before restarting normal control/gamepad services. An abnormal exit reports the problem rather than forcing normal control to resume.
- Leaving control releases that page's connection. Handoff does not automatically energize servos, save calibration, or run policies; the user still chooses actions.

`EASYDUCK_SETUP_PORT` selects setup; `EASYDUCK_CALIBRATION_PORT` defaults to `8766`; `EASYDUCK_CONTROL_PORT` selects control. Page services read setup-port information from `/etc/easyduck/setup.env`. Changing controller/calibration listening ports also requires matching service arguments.

## PS5 gamepad

1. Hold **PS + Create** until the light bar flashes rapidly.
2. Scan and select `Wireless Controller` or `DualSense Wireless Controller`.
3. Choose pair/connect. The service runs `pair`, `trust`, and `connect`, checking actual state.
4. Wake a sleeping/off controller with PS. Paired devices can be connected, disconnected, or forgotten individually. Name, MAC, pairing/connection, battery, and input paths update locally.

Battery is read from BlueZ first, then matching system power devices by MAC; it is hidden if unavailable. Input discovery reads metadata for `/dev/input/js*` and `event*`; it does not open input-event streams, grab devices, or interfere with `padd`.

Trusted, registered devices are retried by a systemd timer after boot. Manual disconnect pauses automatic reconnection for this boot; connect or reboot restores it. Forget removes registration and system pairing.

Missing adapters, pairing mode, authentication failures, busy connections, and pairing timeouts have distinct guidance. The page does not present an unverified guess about another paired host as a confirmed cause.

## Acceptance checks

Stop motion and support the robot before connection tests: an existing gamepad service may react to button presses. Setup does not send robot motion commands.

| Check | Expected result |
| --- | --- |
| Correct Wi-Fi password | Correct SSID/interface/IP; wireless addresses appear |
| Wrong password of valid length | Authentication/password error; old network is not reported as target success |
| DHCP disabled on test hotspot | Association followed by no-IP error, not success |
| Missing SSID | Network-not-found result |
| PS5 pairing | PS + Create, scan/pair; paired and connected state |
| Disconnect/connect | Remains disconnected until connect; existing gamepad reader still works |
| Reconnect | Timer retries after controller wake or board reboot |
| Forget | Pairing/registration removed; pairing required again |
| Battery | Percentage when available, hidden otherwise |

UI checklist:

- [ ] Light/dark theme persists after refresh.
- [ ] Scan, connect, pair, disconnect, and forget do not reload the whole page.
- [ ] Progress/results appear only in bottom-right toasts, without duplicate body notices.
- [ ] Background refresh preserves typed SSID/password.
- [ ] Phone layout has no horizontal overflow and all controls work.
- [ ] Unauthenticated or invalid requests cannot perform connection changes.

## Rollback

Stop newly installed services, then use the installer's backup directory:

```sh
sudo systemctl disable --now easyduck-setup.service easyduck-gamepad-reconnect.timer
sudo systemctl stop easyduck-gamepad-reconnect.service
sudo python3 deploy/rollback.py --root / --backup <BACKUP_DIR> --restore-on-board
sudo systemctl daemon-reload
```

Application rollback does not remove saved Wi-Fi, Bluetooth pairing, or reconnection registration. Forget a pairing through the UI first when needed. Restore any other listener's port according to its original settings.
