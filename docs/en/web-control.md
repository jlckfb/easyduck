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

## Page navigation

The top navigation opens connection setup or physical calibration. It uses the current host's setup service (port 80 by default, from `EASYDUCK_SETUP_PORT` in `/etc/easyduck/setup.env`). Ordinary navigation checks connectivity and retains the current address. After USB disconnects, it can use a previously read Wi-Fi address; without one, it shows connection guidance. Login sessions remain usable on the same host; changing IP or restarting setup requires another login. See [connection setup](connection-setup.md).

Leaving control releases that browser control connection. Entering calibration requires confirmation that the robot is supported, then stops normal control and requests torque release. Leaving calibration checks drafts, motion, and torque state, releases serial ownership, and restores normal services; it does not automatically energize servos or run policies. A successful page switch does not prove physical torque release during communication/power failure.
