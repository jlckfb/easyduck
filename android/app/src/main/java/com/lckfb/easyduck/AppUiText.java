package com.lckfb.easyduck;

final class AppUiText {
    private AppUiText() {}

    static String english(String source) {
        if (source.startsWith("正在连接 ")) {
            return "Connecting to " + source.substring("正在连接 ".length());
        }
        String localControl = " · 局域网遥控";
        if (source.endsWith(localControl)) {
            return source.substring(0, source.length() - localControl.length()) + " · Local control";
        }
        return switch (source) {
            case "尚未连接" -> "Not connected";
            case "连接地址" -> "Address";
            case "横屏" -> "Landscape";
            case "竖屏" -> "Portrait";
            case "切回竖屏" -> "Switch to portrait";
            case "设置" -> "Settings";
            case "亮色" -> "Light theme";
            case "暗色" -> "Dark theme";
            case "对讲已关闭" -> "Intercom off";
            case "开启对讲" -> "Intercom";
            case "按住说话" -> "Hold to talk";
            case "正在打开麦克风" -> "Starting microphone";
            case "取消" -> "Cancel";
            case "请输入有效的局域网 IP" -> "Enter a valid local IP address";
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
