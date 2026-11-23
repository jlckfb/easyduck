package com.lckfb.easyduck;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.media.AudioManager;
import android.net.ConnectivityManager;
import android.net.LinkAddress;
import android.net.LinkProperties;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.net.Uri;
import android.os.Bundle;
import android.text.InputType;
import android.text.TextUtils;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

import java.io.ByteArrayInputStream;
import java.net.Inet4Address;

public final class MainActivity extends Activity {
    private static final int MICROPHONE_PERMISSION = 10;
    private static final int[] VOICE_RETRY_MS = {1000, 2000, 4000, 6000, 8000};
    private static final String RELEASE_CONTROL = "window.dispatchEvent(new Event('blur'));"
            + "window.dispatchEvent(new Event('pagehide'));";
    private static final int INK = Color.rgb(17, 23, 24);
    private static final int PANEL = Color.rgb(29, 41, 42);
    private static final int ACCENT = Color.rgb(216, 248, 139);
    private static final int TEXT = Color.rgb(240, 243, 239);
    private static final int MUTED = Color.rgb(178, 190, 187);

    private WebView web;
    private TextView addressLabel;
    private TextView voiceStatus;
    private TextView routeStatus;
    private Button voiceButton;
    private Button talkButton;
    private volatile String host;
    private boolean controlFailed;
    private VoiceConnection voice;
    private volatile VoiceAudio audio;
    private boolean voiceRequested;
    private boolean voiceWasReady;
    private int voiceRetries;
    private final Runnable retryVoice = this::connectVoice;
    private boolean foreground;
    private boolean permissionGrantedPending;
    private int activePointer = -1;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        buildLayout();
        host = getPreferences(MODE_PRIVATE).getString("host", "");
        if (!isLocalIp(host)) {
            addressLabel.setText("尚未连接");
            showAddressDialog();
        }
    }

    private int dp(int size) {
        return Math.round(size * getResources().getDisplayMetrics().density);
    }

    private TextView text(String label, int size, int color) {
        TextView view = new TextView(this);
        view.setText(label);
        view.setTextSize(size);
        view.setTextColor(color);
        view.setShadowLayer(dp(2), 0, dp(1), Color.BLACK);
        return view;
    }

    private Button button(String label) {
        Button value = new Button(this);
        value.setAllCaps(false);
        value.setMinHeight(dp(48));
        value.setText(label);
        value.setTextColor(TEXT);
        styleButton(value, false);
        return value;
    }

    private void styleButton(Button button, boolean primary) {
        GradientDrawable panel = new GradientDrawable();
        panel.setCornerRadius(dp(10));
        panel.setColor(primary ? 0x76334522 : 0x8C202B2B);
        panel.setStroke(dp(1), primary ? ACCENT : 0x997E9190);
        button.setBackground(new RippleDrawable(ColorStateList.valueOf(0x44FFFFFF), panel, null));
    }

    private void buildLayout() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setBackgroundColor(INK);

        LinearLayout header = new LinearLayout(this);
        header.setPadding(dp(12), 0, dp(8), 0);
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.setBackgroundColor(PANEL);
        LinearLayout headings = new LinearLayout(this);
        headings.setOrientation(LinearLayout.VERTICAL);
        TextView title = text("EasyDuck", 18, ACCENT);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        headings.addView(title);
        addressLabel = text("", 12, MUTED);
        addressLabel.setMaxLines(2);
        addressLabel.setEllipsize(TextUtils.TruncateAt.END);
        headings.addView(addressLabel);
        header.addView(headings, new LinearLayout.LayoutParams(0, -2, 1));
        Button address = button("连接地址");
        address.setOnClickListener(view -> showAddressDialog());
        header.addView(address);
        layout.addView(header);

        web = new WebView(this);
        web.setBackgroundColor(INK);
        web.getSettings().setJavaScriptEnabled(true);
        web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setAllowFileAccess(false);
        web.getSettings().setAllowContentAccess(false);
        web.setWebChromeClient(new WebChromeClient() {
            @Override public void onPermissionRequest(android.webkit.PermissionRequest request) {
                request.deny();
            }
        });
        web.setWebViewClient(new WebViewClient() {
            @Override public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                return !boardUrl(request.getUrl());
            }

            @Override public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                if (boardUrl(request.getUrl())) return null;
                return new WebResourceResponse("text/plain", "utf-8", 403, "Local device only",
                        null, new ByteArrayInputStream(new byte[0]));
            }

            @Override public void onPageFinished(WebView view, String url) {
                if (!boardUrl(Uri.parse(url))) return;
                if (!foreground) view.evaluateJavascript(RELEASE_CONTROL, null);
                if (controlFailed) return;
                addressLabel.setText(host + " · 局域网遥控");
            }

            @Override public void onReceivedHttpError(WebView view, WebResourceRequest request,
                    WebResourceResponse response) {
                if (request.isForMainFrame() && boardUrl(request.getUrl())) displayConnectionError();
            }

            @Override public void onReceivedError(WebView view, WebResourceRequest request,
                    android.webkit.WebResourceError error) {
                if (request.isForMainFrame() && boardUrl(request.getUrl())) {
                    displayConnectionError();
                }
            }
        });
        layout.addView(web, new LinearLayout.LayoutParams(-1, 0, 1));

        LinearLayout footer = new LinearLayout(this);
        footer.setOrientation(LinearLayout.VERTICAL);
        footer.setBackgroundColor(PANEL);
        footer.setPadding(dp(10), dp(4), dp(10), dp(4));
        LinearLayout state = new LinearLayout(this);
        voiceStatus = text("对讲已关闭", 13, TEXT);
        routeStatus = text("", 12, ACCENT);
        state.addView(voiceStatus, new LinearLayout.LayoutParams(0, -2, 1));
        state.addView(routeStatus);
        footer.addView(state);
        LinearLayout actions = new LinearLayout(this);
        voiceButton = button("开启对讲");
        voiceButton.setOnClickListener(view -> {
            if (voiceRequested) stopVoice("对讲已关闭", "manual");
            else enableVoice();
        });
        actions.addView(voiceButton);
        talkButton = button("按住说话");
        talkButton.setTextColor(new ColorStateList(
                new int[][]{{android.R.attr.state_enabled}, {}}, new int[]{ACCENT, MUTED}));
        styleButton(talkButton, true);
        talkButton.setEnabled(false);
        talkButton.setOnTouchListener(this::handleTalkTouch);
        talkButton.setOnClickListener(view -> {
            VoiceAudio session = audio;
            if (session != null && session.isFocusPaused()) session.resumeListening();
        });
        talkButton.setOnKeyListener((view, key, event) -> {
            if (key != KeyEvent.KEYCODE_ENTER && key != KeyEvent.KEYCODE_SPACE) return false;
            VoiceAudio session = audio;
            if (session == null) return false;
            if (session.isFocusPaused()) {
                if (event.getAction() == KeyEvent.ACTION_UP) talkButton.performClick();
                return true;
            }
            if (event.getAction() == KeyEvent.ACTION_DOWN && event.getRepeatCount() == 0) beginTalk(session);
            else if (event.getAction() == KeyEvent.ACTION_UP) session.endTalk();
            return true;
        });
        actions.addView(talkButton, new LinearLayout.LayoutParams(0, -2, 1));
        footer.addView(actions);
        layout.addView(footer);
        setContentView(layout);
    }

    private boolean handleTalkTouch(View button, MotionEvent event) {
        VoiceAudio session = audio;
        if (session == null || !button.isEnabled()) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                activePointer = event.getPointerId(0);
                if (!session.isFocusPaused()) beginTalk(session);
                break;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_POINTER_UP:
                if (event.getPointerId(event.getActionIndex()) == activePointer) {
                    activePointer = -1;
                    if (session.isFocusPaused()) button.performClick();
                    else session.endTalk();
                }
                break;
            case MotionEvent.ACTION_MOVE:
                int index = event.findPointerIndex(activePointer);
                if (index >= 0 && event.getX(index) >= 0 && event.getX(index) < button.getWidth()
                        && event.getY(index) >= 0 && event.getY(index) < button.getHeight()) break;
                // The original finger left the button or was lost.
            case MotionEvent.ACTION_CANCEL:
                if (activePointer != -1) {
                    activePointer = -1;
                    session.endTalk();
                }
                break;
            default:
                break;
        }
        return true;
    }

    private void beginTalk(VoiceAudio session) {
        if (!foreground || !hasWindowFocus()) return;
        voiceStatus.setText("正在打开麦克风");
        session.beginTalk();
    }

    private boolean boardUrl(Uri url) {
        return host != null && host.equals(url.getHost())
                && ("http".equals(url.getScheme()) || "https".equals(url.getScheme()));
    }

    static boolean isLocalIp(String address) {
        if (address == null || !address.matches("[0-9]{1,3}(?:\\.[0-9]{1,3}){3}")) return false;
        String[] parts = address.split("\\.");
        for (String part : parts) {
            if (Integer.parseInt(part) > 255 || (part.length() > 1 && part.startsWith("0"))) return false;
        }
        int first = Integer.parseInt(parts[0]);
        int second = Integer.parseInt(parts[1]);
        return first == 10 || (first == 172 && second >= 16 && second <= 31)
                || (first == 192 && second == 168) || (first == 169 && second == 254);
    }

    private void showAddressDialog() {
        EditText input = new EditText(this);
        input.setInputType(InputType.TYPE_CLASS_PHONE);
        input.setSingleLine();
        input.setText(host == null ? "" : host);
        input.setHint("例如 192.168.1.100");
        AlertDialog dialog = new AlertDialog.Builder(this).setTitle("鸭子的局域网 IP")
                .setView(input).setNegativeButton("取消", null)
                .setPositiveButton("连接", null).create();
        dialog.setOnShowListener(event -> dialog.getButton(AlertDialog.BUTTON_POSITIVE)
                .setOnClickListener(view -> {
                    String candidate = input.getText().toString().trim();
                    if (!isLocalIp(candidate)) {
                        input.setError("请输入有效的局域网 IP");
                        return;
                    }
                    stopVoice("对讲已关闭", "address_change");
                    dialog.dismiss();
                    if (web.getUrl() == null) selectController(candidate);
                    else web.evaluateJavascript(RELEASE_CONTROL, result -> {
                        if (!isDestroyed()) selectController(candidate);
                    });
                }));
        dialog.show();
    }

    private void selectController(String candidate) {
        host = candidate;
        getPreferences(MODE_PRIVATE).edit().putString("host", host).apply();
        if (foreground) openController();
    }

    private Network localWifi(String host) {
        ConnectivityManager manager = getSystemService(ConnectivityManager.class);
        int target = 0;
        for (String part : host.split("\\.")) target = (target << 8) | Integer.parseInt(part);
        Network wifi = null;
        for (Network network : manager.getAllNetworks()) {
            NetworkCapabilities caps = manager.getNetworkCapabilities(network);
            LinkProperties link = manager.getLinkProperties(network);
            if (caps == null || link == null || !caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) continue;
            wifi = network;
            for (LinkAddress address : link.getLinkAddresses()) {
                if (!(address.getAddress() instanceof Inet4Address) || address.getPrefixLength() == 0) continue;
                int local = 0;
                for (byte part : address.getAddress().getAddress()) local = (local << 8) | (part & 255);
                int mask = -1 << (32 - address.getPrefixLength());
                if ((local & mask) == (target & mask)) return network;
            }
        }
        return wifi;
    }

    private void bindLocalWifi() {
        ConnectivityManager manager = getSystemService(ConnectivityManager.class);
        if (!manager.bindProcessToNetwork(localWifi(host))) manager.bindProcessToNetwork(null);
    }

    private void displayConnectionError() {
        controlFailed = true;
        addressLabel.setText("无法连到鸭子，请核对 Wi-Fi 和地址");
    }

    private void openController() {
        bindLocalWifi();
        controlFailed = false;
        addressLabel.setText("正在连接 " + host);
        web.loadUrl("http://" + host + ":9527/");
    }

    private void enableVoice() {
        if (!isLocalIp(host)) {
            showAddressDialog();
            return;
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, MICROPHONE_PERMISSION);
            return;
        }
        voiceRequested = true;
        voiceWasReady = false;
        voiceRetries = 0;
        connectVoice();
    }

    private void connectVoice() {
        if (!foreground || !voiceRequested || voice != null) return;
        bindLocalWifi();
        final String address = host;
        voiceStatus.setText("正在连接对讲");
        voiceButton.setText("取消");
        try {
            VoiceConnection.Listener listener = new VoiceConnection.Listener() {
                @Override public void ready() {
                    runOnUiThread(() -> onVoiceReady(this));
                }
                @Override public void audio(byte[] pcm) {
                    VoiceAudio output = audio;
                    if (output != null && this == voiceListener) output.receive(pcm);
                }
                @Override public void failed(String reason) {
                    runOnUiThread(() -> {
                        if (this != voiceListener) return;
                        voiceConnectionLost(reason);
                    });
                }
            };
            voice = new VoiceConnection(address, listener);
            voiceListener = listener;
            voice.open();
        } catch (Exception error) {
            voiceConnectionLost("无法开启对讲，请检查连接地址");
        }
    }

    private volatile VoiceConnection.Listener voiceListener;

    private void onVoiceReady(VoiceConnection.Listener owner) {
        if (!foreground || voice == null || owner != voiceListener) return;
        try {
            audio = new VoiceAudio(this, voice, new VoiceAudio.Listener() {
                @Override public void listening() {
                    runOnUiThread(() -> { if (owner == voiceListener && audio != null && !audio.isTalking() && !audio.isFocusPaused()) {
                        voiceStatus.setText("正在收听鸭子周围的声音");
                        talkButton.setText("按住说话");
                    } });
                }
                @Override public void focusPaused() {
                    runOnUiThread(() -> {
                        if (owner != voiceListener || audio == null || !audio.isFocusPaused()) return;
                        activePointer = -1;
                        voiceStatus.setText("手机音频暂被占用，对讲保持连接");
                        talkButton.setText("恢复收听");
                    });
                }

                @Override public void speaking() {
                    runOnUiThread(() -> { if (owner == voiceListener && audio != null && audio.isTalking()) {
                        voiceStatus.setText("正在说话");
                        talkButton.setText("松开收听");
                    } });
                }
                @Override public void route(boolean speaker) {
                    runOnUiThread(() -> { if (owner == voiceListener) {
                        routeStatus.setText(speaker ? "手机外放" : "手机未使用外放");
                    } });
                }
                @Override public void failed(String reason, String code) {
                    runOnUiThread(() -> {
                        if (owner != voiceListener) return;
                        if ("connection_error".equals(code)) voiceConnectionLost(reason);
                        else stopVoice(reason, code);
                    });
                }
            });
            setVolumeControlStream(AudioManager.STREAM_VOICE_CALL);
            voiceStatus.setText("正在收听鸭子周围的声音");
            voiceWasReady = true;
            voiceRetries = 0;
            voiceButton.setText("关闭对讲");
            talkButton.setEnabled(true);
        } catch (RuntimeException error) {
            stopVoice("无法启用手机扬声器", "playback_error");
        }
    }

    private void voiceConnectionLost(String reason) {
        if (!foreground || !voiceRequested || !voiceWasReady || voiceRetries == VOICE_RETRY_MS.length) {
            stopVoice(reason, "connection_error");
            return;
        }
        releaseVoice("连接中断，正在重连", "connection_error");
        voiceButton.setText("取消重连");
        web.postDelayed(retryVoice, VOICE_RETRY_MS[voiceRetries++]);
    }

    private void stopVoice(String reason, String code) {
        voiceRequested = false;
        web.removeCallbacks(retryVoice);
        releaseVoice(reason, code);
    }

    private void releaseVoice(String reason, String code) {
        voiceListener = null;
        VoiceAudio output = audio;
        audio = null;
        if (output != null) output.close();
        if (voice != null) voice.close(code);
        voice = null;
        activePointer = -1;
        voiceButton.setText("开启对讲");
        talkButton.setText("按住说话");
        talkButton.setEnabled(false);
        voiceStatus.setText(reason);
        routeStatus.setText("");
        setVolumeControlStream(AudioManager.USE_DEFAULT_STREAM_TYPE);
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request != MICROPHONE_PERMISSION) return;
        if (results.length == 0 || results[0] != PackageManager.PERMISSION_GRANTED) {
            voiceStatus.setText("开启对讲需要麦克风权限");
            return;
        }
        if (foreground) enableVoice();
        else permissionGrantedPending = true;
    }

    @Override protected void onResume() {
        super.onResume();
        foreground = true;
        web.onResume();
        if (isLocalIp(host)) openController();
        if (permissionGrantedPending) {
            permissionGrantedPending = false;
            web.post(this::enableVoice);
        }
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        VoiceAudio session = audio;
        if (!focused && session != null && session.isTalking()) {
            activePointer = -1;
            session.endTalk();
        }
    }

    @Override protected void onPause() {
        foreground = false;
        stopVoice("对讲已关闭", "app_paused");
        web.evaluateJavascript(RELEASE_CONTROL, null);
        web.onPause();
        super.onPause();
    }

    @Override protected void onDestroy() {
        getSystemService(ConnectivityManager.class).bindProcessToNetwork(null);
        web.destroy();
        super.onDestroy();
    }
}
