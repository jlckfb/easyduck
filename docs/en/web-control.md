# Web controller

**English** | [简体中文](../zh-CN/web-control.md)

A Python 3.9+ standard-library service on port 9527. It connects to the daemon through `/run/robotd.sock` and does not open the servo serial port.

`xgoduck-web.service` starts it on the board. Open `http://BOARD_IP:9527`, choose **用网页控制** (use web control), then explicitly enable walking.

- Left stick: forward/backward and turning; release to stop. W/S move forward/backward, A/D turn. No lateral movement. Default speed is 50%.
- Right stick: head pitch/yaw. Two sliders independently control neck pitch and head roll; a separate button recenters head commands.
- Sit/stand and pickup depend on available models. The browser exposes neither roulade nor jumping; its API rejects both.
- Hold vocalization: reuse the gamepad's wheee sound, ramping to full level and mouth opening over about 0.35 seconds. Release stops sound and sends mouth-close immediately; each new press starts at zero. Audio gain follows squared intensity to avoid loud sound at small mouth openings. Blur, hidden pages, or expired input stop vocalization. Older gamepad requests without an audio-level field retain their previous volume behavior.

Default pose is at the bottom of the central control area. The full-width details section below shows joint data and logs; torque release remains available. Deployment and offline tests do not perform physical operations for the user.

Run in the repository's `web/` directory:

```sh
python3 server.py --help
python3 -m unittest discover -s tests -v
node tests/controls.test.cjs
```

Tests use mock RPC. See the [software guide](software-guide.md) for installation/startup.

## Android app control and intercom

1. Connect the calibrated EasyDuck to Wi-Fi and listen for its spoken IP. Connect the phone to the same Wi-Fi.
2. Open the EasyDuck Android app and enter **the duck's spoken IP** under **连接地址** (Connection address). Take control as usual. The app remembers the address. If the duck changes networks or receives a new IP, enter the latest address it announces.
3. With a microphone on the expansion board, tap **开启对讲** (Start intercom). Allow microphone access on first use. Hold to talk and release to hear the duck's surroundings. The app requests playback through the phone's loudspeaker.

Control and voice stay on the local network. The Wi-Fi works without internet access. Intercom uses the duck's IP. No QR scan or pairing code is required. Devices that can reach the duck on the LAN can start intercom, with one active voice session at a time.

For a source-built board, install the [Android intercom dependencies](../../web/requirements-intercom.txt) once from the repository root on the board, then follow the [software guide](software-guide.md) to install and start the services. Python pip must be available on the board.

```sh
sudo python3 -m pip install --target /opt/robot/xgoduck-web/intercom-deps -r web/requirements-intercom.txt
```

To build the APK, run the following from the repository root on a computer with Android SDK 33 and Gradle 8.7:

```sh
gradle -p android :app:assembleDebug
```

The output is `android/app/build/outputs/apk/debug/app-debug.apk`. The same APK can connect to different ducks running the Android intercom service.

## RGB status indicator

Use the **状态指示灯** (Status indicator) section to turn the WS2812B light on or off, adjust brightness and battery breathing speed, and see the current pattern. The light defaults to on, at 70% brightness and the fastest battery breathing speed (level 5). The speed setting applies only to the green, yellow, and red battery breathing patterns.

| Light pattern | Meaning |
| --- | --- |
| Green breathing | Battery level ≥ 50% |
| Yellow breathing | 20% ≤ battery level < 50% |
| Red breathing | Battery level < 20% |
| Slow blue breathing | The calibration required for walking has yet to be saved or has failed validation. Battery display resumes when the calibration is valid. Pending mechanical limit confirmations remain visible in the calibration workbench. |
| Two short white flashes | Gamepad connected |
| White on for about 1 second | Gamepad disconnected |
| Repeating purple double flash | The robot daemon keeps reporting a fault, or the indicator service cannot read the daemon's status for a sustained period. Check the logs in **详细状态** (Detailed status). |
| Repeating rapid red flashes | [Low-battery protection](low-battery.md) is active and actions are temporarily locked. |
| Off | The indicator is disabled or a valid battery reading is temporarily unavailable. |

When enabled, the indicator shows low-battery protection first, followed by persistent faults, gamepad connection or disconnection, pending calibration, and battery level. Once low-battery protection clears, rapid red flashes stop and the indicator shows the applicable state.

## Page navigation

The top navigation opens connection setup or physical calibration. It uses the current host's setup service (port 80 by default, from `EASYDUCK_SETUP_PORT` in `/etc/easyduck/setup.env`). Ordinary navigation checks connectivity and retains the current address. After USB disconnects, it can use a previously read Wi-Fi address; without one, it shows connection guidance. Login sessions remain usable on the same host; changing IP or restarting setup requires another login. See [connection setup](connection-setup.md).

Leaving control releases that browser control connection. Entering calibration requires confirmation that the robot is supported, then stops normal control and requests torque release. Leaving calibration checks drafts, motion, and torque state, releases serial ownership, and restores normal services; it does not automatically energize servos or run policies. A successful page switch does not prove physical torque release during communication/power failure.
