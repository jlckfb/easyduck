package com.lckfb.easyduck;

final class AppUiText {
    private AppUiText() {}

    static String english(String source) {
        if (source.startsWith("正在连接 ")) {
            return "Connecting to " + source.substring("正在连接 ".length());
        }
        if (source.startsWith("手机和更新服务器连接同一 Wi-Fi，端口 ")) {
            return "Connect your phone to the update server’s Wi-Fi. Port " + source.substring("手机和更新服务器连接同一 Wi-Fi，端口 ".length());
        }
        if (source.startsWith("没有更新，当前版本 ")) {
            return "You’re up to date. Installed version: " + source.substring("没有更新，当前版本 ".length());
        }
        if (source.startsWith("发现新版本 ")) {
            return "Update available: " + source.substring("发现新版本 ".length());
        }
        String localControl = " · 局域网遥控";
        if (source.endsWith(localControl)) {
            return source.substring(0, source.length() - localControl.length()) + " · Local control";
        }
        return switch (source) {
            case "尚未连接" -> "Not connected";
            case "连接地址" -> "Address";
            case "检查更新" -> "Updates";
            case "横屏" -> "Landscape";
            case "竖屏" -> "Portrait";
            case "切回竖屏" -> "Switch to portrait";
            case "设置" -> "Settings";
            case "亮色" -> "Light theme";
            case "暗色" -> "Dark theme";
            case "对讲已关闭" -> "Intercom off";
            case "音频" -> "Audio";
            case "鸭子收音" -> "Duck microphone";
            case "鸭子喇叭音量" -> "Duck speaker volume";
            case "同时调整鸭子的对讲和提示音音量。" -> "Adjusts the duck’s intercom and sound prompts.";
            case "无法读取鸭子喇叭音量，请检查连接和板端版本" ->
                    "Could not read speaker volume. Check the connection and duck software version.";
            case "鸭子未能调整喇叭音量" -> "The duck could not change its speaker volume.";
            case "应用" -> "Apply";
            case "鸭子收音强度" -> "Duck microphone level";
            case "调节鸭子麦克风的硬件增益。档位越高，环境声和底噪都会变响。" ->
                    "Set the duck’s hardware microphone gain. Higher settings amplify ambient sound and noise.";
            case "鸭子端需要更新后才能调硬件增益" -> "Update the duck software to adjust hardware gain.";
            case "鸭子未能切换麦克风增益" -> "The duck could not change its microphone gain.";
            case "完成" -> "Done";
            case "开启对讲" -> "Intercom";
            case "按住说话" -> "Hold to talk";
            case "正在打开麦克风" -> "Starting microphone";
            case "测试电脑或板端的局域网 IP" -> "Update server IP";
            case "局域网检查更新" -> "Check for local updates";
            case "取消" -> "Cancel";
            case "检查" -> "Check";
            case "请输入有效的局域网 IP" -> "Enter a valid local IP address";
            case "检查中" -> "Checking…";
            case "无法读取当前 App 版本" -> "Could not read the installed app version";
            case "当前版本 %s。下载后按手机提示覆盖安装。" -> "Installed version: %s. Download the APK and follow Android’s prompts to update.";
            case "稍后" -> "Later";
            case "下载 APK" -> "Download APK";
            case "下载中" -> "Downloading…";
            case "安装包已下载" -> "APK downloaded";
            case "按 Android 提示覆盖安装，原来的鸭子地址会保留。" -> "Follow Android’s prompts to update. Your duck’s address will be kept.";
            case "安装" -> "Install";
            case "允许 EasyDuck 安装测试包" -> "Allow EasyDuck to install test updates";
            case "在系统设置里打开「允许来自此来源」，返回后继续安装。" -> "Turn on Allow from this source in Android settings, then return to install.";
            case "去设置" -> "Open settings";
            case "找不到系统安装权限设置" -> "Could not open Android’s install permission settings";
            case "安装包已被清理，请重新下载" -> "The downloaded APK was removed. Download it again.";
            case "无法打开 Android 安装界面" -> "Could not open the Android installer";
            case "例如 192.168.1.100" -> "e.g. 192.168.1.100";
            case "鸭子的局域网 IP" -> "Duck’s local IP address";
            case "连接" -> "Connect";
            case "无法连到鸭子，请核对 Wi-Fi 和地址" -> "Could not reach the duck. Check Wi-Fi and the address.";
            case "正在连接对讲" -> "Connecting intercom";
            case "无法开启对讲，请检查连接地址" -> "Could not start intercom. Check the address.";
            case "正在收听鸭子周围的声音" -> "Listening to the duck";
            case "手机音频暂被占用，对讲保持连接" -> "Phone audio is busy. Intercom is still connected.";
            case "恢复收听" -> "Resume listening";
            case "正在说话" -> "Talking";
            case "松开收听" -> "Release to listen";
            case "手机外放" -> "Speaker on";
            case "手机未使用外放" -> "Speaker off";
            case "关闭对讲" -> "End intercom";
            case "无法启用手机扬声器" -> "Could not start the phone speaker";
            case "连接中断，正在重连" -> "Connection lost. Reconnecting…";
            case "取消重连" -> "Cancel retry";
            case "开启对讲需要麦克风权限" -> "Allow microphone access to use intercom.";
            case "无法读取更新信息，请检查局域网地址和 update.json" -> "Could not check for updates. Check the local IP address and update.json.";
            case "APK 下载或校验失败，请检查安装包、版本和签名" -> "APK download or verification failed. Check the file, version and signature.";
            case "手机扬声器播放失败" -> "Phone speaker playback failed";
            case "对讲连接已断开" -> "Intercom disconnected";
            case "手机麦克风无法录音，请检查权限" -> "Could not record audio. Check microphone access.";
            case "无法连接鸭子的对讲服务，请检查 Wi-Fi 和鸭子的 IP" -> "Could not reach the duck’s intercom. Check Wi-Fi and the duck’s IP.";
            case "另一台手机正在使用对讲" -> "Another phone is using the intercom";
            case "请稍等一秒后重试" -> "Wait a second and try again";
            case "鸭子的对讲服务暂时不可用" -> "The duck’s intercom is unavailable";
            case "鸭子的对讲响应有误" -> "Invalid response from the duck’s intercom";
            case "无法开启对讲" -> "Could not start intercom";
            case "对讲已断开，请重新开启" -> "Intercom disconnected. Start it again.";
            case "对讲已断开，请检查同一个 Wi-Fi 后重试" -> "Intercom disconnected. Check that both devices use the same Wi-Fi.";
            case "对讲连接超时" -> "Intercom connection timed out";
            default -> source;
        };
    }
}
