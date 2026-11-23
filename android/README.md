# EasyDuck Android

[简体中文](README.zh-CN.md)

The EasyDuck Android app lets you control a calibrated robot and use push-to-talk on the same Wi-Fi. Control and audio work without internet access. The app remembers the duck's local IP address.

## Use the app

1. Connect the calibrated duck to Wi-Fi. Listen for the IP address it announces, then connect the phone to the same Wi-Fi.
2. Install the APK on a phone running Android 8.0 (API 26) or later. Open EasyDuck, tap **连接地址** (Connection address), enter the duck's IP, then tap **连接** (Connect).
3. On the controller page, tap **用网页控制** (Take web control). If the duck's IP changes, enter the newly announced address under **连接地址**.
4. With a microphone connected to the duck, tap **开启对讲** (Start intercom) and allow microphone access on first use. Hold **按住说话** (Hold to talk) to speak through the duck's speaker. Release the button to listen through the phone's loudspeaker.

The app connects directly to the duck by IP. There is no pairing code or certificate setup. Devices on the LAN that can reach the duck can start intercom. One session can be active at a time.

## Deploy on TaishanPi

For a board that already runs a compatible EasyDuck installation, check that the web controller on port **9527** and the Android intercom service on port **9460** are available. For a source installation, prepare the ARM64 binaries and system dependencies according to the [deployment guide](../docs/en/deployment.md#install-on-a-board). Support the duck and stop the control services before installing.

On the TaishanPi, run from the repository root. Python 3 and pip must be available.

```sh
sudo python3 deploy/install.py --root / --profile board --install-on-board
sudo python3 -m pip install --target /opt/robot/xgoduck-web/intercom-deps -r web/requirements-intercom.txt
```

The installer keeps existing configuration under `/etc/robot/`. On an upgraded board, check that `[sound].device` in `/etc/robot/robotd.toml` is `"easyduck_speaker"` so prompts and intercom share the speaker. Preserve the robot's own calibration files.

After calibration, support the duck before starting the services. Run from the repository root.

```sh
sudo bash deploy/start.sh --execute
systemctl is-active xgoduck-web.service easyduck-android-intercom.service
curl --fail http://127.0.0.1:9460/health
```

## Build the APK

Install **JDK 17**, **Android SDK Platform 33**, and **Gradle 8.7** on your build computer. Put Gradle on your PATH and set `JAVA_HOME` and `ANDROID_HOME` to the JDK and Android SDK directories. The first build requires access to Google's Maven repository and Maven Central to resolve dependencies.

Run from the repository root.

```sh
gradle -p android :app:assembleDebug
```

The APK is written to `android/app/build/outputs/apk/debug/app-debug.apk`. Copy it to your phone and open it to install. To update an existing installation in place, sign the new APK with the same key.

## Source layout

| File | Responsibility |
| --- | --- |
| [Gradle module](app/build.gradle) and [manifest](app/src/main/AndroidManifest.xml) | APK build, supported Android versions, and permissions |
| [MainActivity.java](app/src/main/java/com/lckfb/easyduck/MainActivity.java) | Remembers the duck's IP and opens the board's control page in a WebView |
| [VoiceConnection.java](app/src/main/java/com/lckfb/easyduck/VoiceConnection.java) and [VoiceAudio.java](app/src/main/java/com/lckfb/easyduck/VoiceAudio.java) | Connect to the board's intercom and handle the phone's microphone and speaker |
| [Board intercom](../web/android_intercom.py) | Exchanges audio with the app and uses the duck's microphone and shared speaker |

The app displays the board's [web controller](../docs/en/web-control.md) and handles Android audio. Robot motion and calibration are handled on the board.
