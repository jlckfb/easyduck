# EasyDuck Android

[English](README.md)

EasyDuck Android App 可以在同一个 Wi-Fi 内遥控已经标定的鸭子，并使用按住说话对讲。遥控和对讲都能在没有互联网的情况下使用。App 会记住鸭子的局域网 IP。

## 使用 App

1. 让标定完成的鸭子连接 Wi-Fi，听它播报 IP。手机也连到同一个 Wi-Fi。
2. 在 Android 8.0（API 26）或更新版本的手机上安装 APK。打开 EasyDuck，点「连接地址」，填鸭子的 IP，再点「连接」。
3. 在遥控页点「用网页控制」。鸭子换了 IP，就在「连接地址」填它新播报的地址。
4. 鸭子接有麦克风时，点「开启对讲」，第一次使用时允许手机录音。按住「按住说话」，鸭子会播放手机传过去的声音。松开后从手机扬声器听鸭子周围的声音。

App 直接通过 IP 连鸭子，无需配对码或配置证书。能连到鸭子的局域网设备都可以发起对讲，同一时间只能开一个对讲会话。

## 在泰山派上部署

如果板端已经安装了兼容版本，确认 **9527** 端口的网页遥控和 **9460** 端口的 Android 对讲服务可用。需要从源码安装时，按[部署说明](../docs/zh-CN/deployment.md)准备 ARM64 程序和系统依赖。安装前把鸭子托稳并停止控制服务。

以下命令在泰山派的仓库根目录执行。板端需要 Python 3 和 pip。

```sh
sudo python3 deploy/install.py --root / --profile board --install-on-board
sudo python3 -m pip install --target /opt/robot/xgoduck-web/intercom-deps -r web/requirements-intercom.txt
```

安装会保留 `/etc/robot/` 中的现有配置。旧设备覆盖安装后，检查 `/etc/robot/robotd.toml` 里 `[sound].device` 是否为 `"easyduck_speaker"`，这样提示音和对讲才能共用扬声器。保留设备原有的标定文件。

完成标定并托稳鸭子后，在仓库根目录启动并检查服务。

```sh
sudo bash deploy/start.sh --execute
systemctl is-active xgoduck-web.service easyduck-android-intercom.service
curl --fail http://127.0.0.1:9460/health
```

## 编译 APK

在编译电脑上安装 **JDK 17**、**Android SDK Platform 33** 和 **Gradle 8.7**。把 Gradle 加到 PATH，并将 `JAVA_HOME` 和 `ANDROID_HOME` 分别指向 JDK 和 Android SDK 目录。首次构建需要访问 Google Maven 和 Maven Central 下载依赖。

在仓库根目录执行。

```sh
gradle -p android :app:assembleDebug
```

生成的 APK 在 `android/app/build/outputs/apk/debug/app-debug.apk`。复制到手机后打开安装。要覆盖更新手机上的现有版本，新 APK 需要沿用原来的签名密钥。

## 源码说明

| 文件 | 用途 |
| --- | --- |
| [Gradle 模块](app/build.gradle)和 [Android 清单](app/src/main/AndroidManifest.xml) | APK 构建、Android 版本要求和权限 |
| [MainActivity.java](app/src/main/java/com/lckfb/easyduck/MainActivity.java) | 记住鸭子 IP，在 App 内打开板端遥控页 |
| [VoiceConnection.java](app/src/main/java/com/lckfb/easyduck/VoiceConnection.java) 和 [VoiceAudio.java](app/src/main/java/com/lckfb/easyduck/VoiceAudio.java) | 连接板端对讲，管理手机麦克风和扬声器 |
| [板端对讲服务](../web/android_intercom.py) | 和 App 交换音频，使用鸭子的麦克风和共享扬声器 |

App 显示板端的[网页遥控器](../docs/zh-CN/web-control.md)，并处理手机音频。动作执行和标定由板端处理。
