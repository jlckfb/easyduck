package com.lckfb.easyduck;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.content.res.Configuration;
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
import android.provider.Settings;
import android.text.InputType;
import android.text.TextUtils;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.SeekBar;
import android.widget.TextView;
import android.widget.Toast;

import java.io.ByteArrayInputStream;
import java.net.Inet4Address;

import okhttp3.Call;
import org.json.JSONArray;
import org.json.JSONException;

public final class MainActivity extends Activity {
    private static final int SPEAKER_VOLUME_STEP = 5;
    private static final int MICROPHONE_PERMISSION = 10;
    private static final int INSTALL_PERMISSION = 11;
    private static final int[] VOICE_RETRY_MS = {1000, 2000, 4000, 6000, 8000};
    private static final String RELEASE_CONTROL = "window.dispatchEvent(new Event('blur'));"
            + "window.dispatchEvent(new Event('pagehide'));";
    private static final int INK = Color.rgb(17, 23, 24);
    private static final int PANEL = Color.rgb(29, 41, 42);
    private static final int ACCENT = Color.rgb(216, 248, 139);
    private static final int TEXT = Color.rgb(240, 243, 239);
    private static final int MUTED = Color.rgb(178, 190, 187);

    private String language;
    private String theme;
    private TextView title;
    private WebView web;
    private TextView addressLabel;
    private TextView voiceStatus;
    private TextView routeStatus;
    private Button voiceButton;
    private Button talkButton;
    private Button audioSettingsButton;
    private int micBoost;
    private Call speakerVolumeCall;
    private Button updateButton;
    private Button orientationButton;
    private Button addressButton;
    private Button landscapeSettings;
    private LinearLayout appHeader;
    private LinearLayout voiceFooter;
    private boolean overlaySupported;
    private LinearLayout voiceState;
    private LinearLayout voiceActions;
    private Call updateCall;
    private boolean installPending;
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
        language = getPreferences(MODE_PRIVATE).getString("ui-language", null);
        theme = getPreferences(MODE_PRIVATE).getString("ui-theme", null);
        micBoost = Math.max(0, Math.min(3, getPreferences(MODE_PRIVATE).getInt("mic-boost", 3)));
        buildLayout();
        applyAppearance();
        host = getPreferences(MODE_PRIVATE).getString("host", "");
        if (!isLocalIp(host)) {
            label(addressLabel, "尚未连接");
            showAddressDialog();
        }
    }

    private int dp(int size) {
        return Math.round(size * getResources().getDisplayMetrics().density);
    }

    private boolean light() {
        return "day".equals(theme);
    }

    private boolean english() {
        return language == null ? !"zh".equals(getResources().getConfiguration().getLocales()
                .get(0).getLanguage()) : "en".equals(language);
    }

    private String tr(String source) {
        return english() ? AppUiText.english(source) : source;
    }

    private void label(TextView view, String source) {
        view.setTag(source);
        view.setText(tr(source));
    }

    private ContextThemeWrapper themedContext() {
        return new ContextThemeWrapper(this, light() ? R.style.EasyDuckLightTheme : R.style.EasyDuckTheme);
    }

    private int surfaceColor() {
        return light() ? 0xFFF7FAF2 : PANEL;
    }

    private void applyAppearance() {
        int background = light() ? 0xFFEEF2E8 : INK;
        int textColor = light() ? 0xFF202C28 : TEXT;
        int muted = light() ? 0xFF485D51 : MUTED;
        int accent = light() ? 0xFF375224 : ACCENT;
        web.setBackgroundColor(background);
        getWindow().setStatusBarColor(background);
        getWindow().setNavigationBarColor(background);
        getWindow().getDecorView().setSystemUiVisibility(light()
                ? View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR : 0);
        for (TextView view : new TextView[]{title, addressLabel, voiceStatus, routeStatus,
                addressButton, updateButton, orientationButton, landscapeSettings, voiceButton, audioSettingsButton, talkButton}) {
            view.setText(tr((String) view.getTag()));
            view.setShadowLayer(light() ? 0 : dp(2), 0, dp(1), Color.BLACK);
        }
        title.setTextColor(accent);
        addressLabel.setTextColor(muted);
        voiceStatus.setTextColor(textColor);
        routeStatus.setTextColor(accent);
        for (Button button : new Button[]{addressButton, updateButton, orientationButton,
                landscapeSettings, voiceButton, audioSettingsButton}) {
            button.setTextColor(textColor);
            styleButton(button, false);
        }
        talkButton.setTextColor(new ColorStateList(
                new int[][]{{android.R.attr.state_enabled}, {}}, new int[]{accent, muted}));
        styleButton(talkButton, true);
        updateViewport();
    }

    private void setAppearance(String nextLanguage, String nextTheme, boolean updatePage) {
        if (!"zh-CN".equals(nextLanguage) && !"en".equals(nextLanguage)) return;
        if (!"day".equals(nextTheme) && !"night".equals(nextTheme)) return;
        if (nextLanguage.equals(language) && nextTheme.equals(theme)) return;
        language = nextLanguage;
        theme = nextTheme;
        getPreferences(MODE_PRIVATE).edit().putString("ui-language", language)
                .putString("ui-theme", theme).apply();
        applyAppearance();
        if (updatePage) {
            web.evaluateJavascript("if(typeof controlLocale!=='undefined')controlLocale.setLanguage('"
                    + language + "');if(typeof setTheme==='function')setTheme('" + theme + "');", null);
        }
    }

    private TextView text(String label, int size, int color) {
        TextView view = new TextView(this);
        label(view, label);
        view.setTextSize(size);
        view.setTextColor(color);
        view.setShadowLayer(dp(2), 0, dp(1), Color.BLACK);
        return view;
    }

    private Button button(String label) {
        Button value = new Button(this);
        value.setAllCaps(false);
        value.setMinHeight(dp(48));
        label(value, label);
        value.setTextColor(TEXT);
        styleButton(value, false);
        return value;
    }

    private void styleButton(Button button, boolean primary) {
        GradientDrawable panel = new GradientDrawable();
        panel.setCornerRadius(dp(10));
        panel.setColor(light() ? (primary ? 0xEDF0F6DE : 0xEDFFFFFF)
                : (primary ? 0x76334522 : 0x8C202B2B));
        panel.setStroke(dp(1), light() ? (primary ? 0xFF829B5E : 0xFFA7B6A7)
                : (primary ? ACCENT : 0x997E9190));
        button.setBackground(new RippleDrawable(ColorStateList.valueOf(light() ? 0x27375224 : 0x44FFFFFF), panel, null));
    }

    private void buildLayout() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setBackgroundColor(INK);

        LinearLayout header = appHeader = new LinearLayout(this);
        header.setPadding(dp(12), 0, dp(8), 0);
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.setBackgroundColor(PANEL);
        LinearLayout headings = new LinearLayout(this);
        headings.setOrientation(LinearLayout.VERTICAL);
        title = text("EasyDuck", 18, ACCENT);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        headings.addView(title);
        addressLabel = text("", 12, MUTED);
        addressLabel.setMaxLines(2);
        addressLabel.setEllipsize(TextUtils.TruncateAt.END);
        headings.addView(addressLabel);
        header.addView(headings, new LinearLayout.LayoutParams(0, -2, 1));
        Button address = addressButton = button("连接地址");
        address.setOnClickListener(view -> showAddressDialog());
        header.addView(address);
        updateButton = button("检查更新");
        updateButton.setOnClickListener(view -> checkForUpdate());
        header.addView(updateButton);
        orientationButton = button("横屏");
        orientationButton.setOnClickListener(view -> setRequestedOrientation(isLandscape()
                ? ActivityInfo.SCREEN_ORIENTATION_USER_PORTRAIT
                : ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE));
        header.addView(orientationButton);
        landscapeSettings = button("设置");
        landscapeSettings.setVisibility(View.GONE);
        landscapeSettings.setOnClickListener(this::showLandscapeSettings);
        header.addView(landscapeSettings);
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
                Uri destination = request.getUrl();
                if ("easyduck-ui".equals(destination.getScheme())) {
                    String current = view.getUrl();
                    if (request.isForMainFrame() && "appearance".equals(destination.getHost())
                            && current != null && boardUrl(Uri.parse(current))
                            && Uri.parse(current).getPort() == 9527) {
                        setAppearance(destination.getQueryParameter("lang"),
                                destination.getQueryParameter("theme"), false);
                    }
                    return true;
                }
                return !boardUrl(destination);
            }

            @Override public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                if (boardUrl(request.getUrl())) return null;
                return new WebResourceResponse("text/plain", "utf-8", 403, "Local device only",
                        null, new ByteArrayInputStream(new byte[0]));
            }

            @Override public void onPageStarted(WebView view, String url, android.graphics.Bitmap icon) {
                overlaySupported = false;
                updateViewport();
            }

            @Override public void onPageFinished(WebView view, String url) {
                if (!boardUrl(Uri.parse(url))) return;
                if (!foreground) view.evaluateJavascript(RELEASE_CONTROL, null);
                if (controlFailed) return;
                view.evaluateJavascript("document.querySelector('.intercom-panel')?.setAttribute('hidden','');"
                        + "[document.documentElement.classList.contains('camera-backdrop'),"
                        + "document.documentElement.lang,document.documentElement.dataset.theme];", result -> {
                    if (result == null || !url.equals(view.getUrl())) return;
                    try {
                        JSONArray page = new JSONArray(result);
                        overlaySupported = page.optBoolean(0);
                        if (overlaySupported && (language == null || theme == null)) {
                            setAppearance(language == null ? page.optString(1) : language,
                                    theme == null ? page.optString(2) : theme, false);
                        }
                    } catch (JSONException ignored) {
                        overlaySupported = false;
                    }
                    updateViewport();
                });
                label(addressLabel, host + " · 局域网遥控");
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
        FrameLayout content = new FrameLayout(this);
        content.addView(web, new FrameLayout.LayoutParams(-1, -1));
        layout.addView(new View(this), new LinearLayout.LayoutParams(-1, 0, 1));

        LinearLayout footer = voiceFooter = new LinearLayout(this);
        footer.setOrientation(LinearLayout.VERTICAL);
        footer.setBackgroundColor(PANEL);
        footer.setPadding(dp(10), dp(4), dp(10), dp(4));
        LinearLayout state = voiceState = new LinearLayout(this);
        voiceStatus = text("对讲已关闭", 13, TEXT);
        routeStatus = text("", 12, ACCENT);
        state.addView(voiceStatus, new LinearLayout.LayoutParams(0, -2, 1));
        state.addView(routeStatus);
        footer.addView(state);
        LinearLayout actions = voiceActions = new LinearLayout(this);
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
        audioSettingsButton = button("音频");
        audioSettingsButton.setOnClickListener(this::showAudioSettings);
        actions.addView(audioSettingsButton);
        footer.addView(actions);
        layout.addView(footer);
        content.addView(layout, new FrameLayout.LayoutParams(-1, -1));
        layout.setBackgroundColor(Color.TRANSPARENT);
        setContentView(content);
        View.OnLayoutChangeListener syncViewport = (view, l, t, r, b, oldL, oldT, oldR, oldB) -> {
            if (b - t != oldB - oldT) updateViewport();
        };
        header.addOnLayoutChangeListener(syncViewport);
        footer.addOnLayoutChangeListener(syncViewport);
        updateOrientationLayout();
    }

    private boolean isLandscape() {
        return getResources().getConfiguration().orientation == Configuration.ORIENTATION_LANDSCAPE;
    }

    private void updateOrientationLayout() {
        boolean landscape = isLandscape();
        label(orientationButton, landscape ? "竖屏" : "横屏");
        if (landscape) getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        voiceFooter.setOrientation(landscape ? LinearLayout.HORIZONTAL : LinearLayout.VERTICAL);
        voiceFooter.setGravity(Gravity.CENTER_VERTICAL);
        voiceState.setLayoutParams(new LinearLayout.LayoutParams(landscape ? 0 : -1, -2,
                landscape ? 1 : 0));
        voiceActions.setLayoutParams(new LinearLayout.LayoutParams(landscape ? 0 : -1, -2,
                landscape ? 1.5f : 0));
        updateViewport();
    }

    private void showAudioSettings(View anchor) {
        PopupMenu options = new PopupMenu(themedContext(), anchor);
        options.getMenu().add(tr("鸭子收音")).setOnMenuItemClickListener(item -> {
            showMicBoost();
            return true;
        });
        options.getMenu().add(tr("鸭子喇叭音量")).setOnMenuItemClickListener(item -> {
            showSpeakerVolume();
            return true;
        });
        options.show();
    }

    private void showSpeakerVolume() {
        if (!isLocalIp(host)) {
            showAddressDialog();
            return;
        }
        if (speakerVolumeCall != null) return;
        bindLocalWifi();
        String address = host;
        speakerVolumeCall = VoiceConnection.speakerVolume(address, null, current -> runOnUiThread(() -> {
            speakerVolumeCall = null;
            if (!foreground || isDestroyed() || !address.equals(host)) return;
            if (current < 0) {
                Toast.makeText(this, tr("无法读取鸭子喇叭音量，请检查连接和板端版本"), Toast.LENGTH_LONG).show();
                return;
            }
            ContextThemeWrapper context = themedContext();
            LinearLayout content = new LinearLayout(context);
            content.setOrientation(LinearLayout.VERTICAL);
            content.setPadding(dp(24), dp(12), dp(24), 0);
            TextView explanation = new TextView(context);
            explanation.setText(tr("同时调整鸭子的对讲和提示音音量。"));
            content.addView(explanation);
            TextView value = new TextView(context);
            value.setTextSize(20);
            value.setText(current + "%");
            content.addView(value);
            SeekBar slider = new SeekBar(context);
            slider.setContentDescription(tr("鸭子喇叭音量"));
            slider.setMax(100 / SPEAKER_VOLUME_STEP);
            slider.setProgress(current / SPEAKER_VOLUME_STEP);
            slider.setMinimumHeight(dp(48));
            slider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
                @Override public void onProgressChanged(SeekBar bar, int progress, boolean user) {
                    value.setText(progress * SPEAKER_VOLUME_STEP + "%");
                }

                @Override public void onStartTrackingTouch(SeekBar bar) {}
                @Override public void onStopTrackingTouch(SeekBar bar) {}
            });
            content.addView(slider, new LinearLayout.LayoutParams(-1, -2));
            new AlertDialog.Builder(context).setTitle(tr("鸭子喇叭音量")).setView(content)
                    .setNegativeButton(tr("取消"), null)
                    .setPositiveButton(tr("应用"), (dialog, which) -> {
                        int selected = slider.getProgress() * SPEAKER_VOLUME_STEP;
                        if (selected == current) return;
                        speakerVolumeCall = VoiceConnection.speakerVolume(address, selected, applied -> runOnUiThread(() -> {
                            speakerVolumeCall = null;
                            if (applied < 0 && foreground && !isDestroyed()) {
                                Toast.makeText(this, tr("鸭子未能调整喇叭音量"), Toast.LENGTH_LONG).show();
                            }
                        }));
                    }).show();
        }));
    }

    private void showMicBoost() {
        VoiceConnection session = voice;
        if (session != null && audio != null && !session.supportsMicBoost()) {
            Toast.makeText(this, tr("鸭子端需要更新后才能调硬件增益"), Toast.LENGTH_LONG).show();
            return;
        }
        ContextThemeWrapper context = themedContext();
        LinearLayout content = new LinearLayout(context);
        content.setOrientation(LinearLayout.VERTICAL);
        content.setPadding(dp(24), dp(12), dp(24), 0);

        TextView explanation = new TextView(context);
        explanation.setText(tr("调节鸭子麦克风的硬件增益。档位越高，环境声和底噪都会变响。"));
        content.addView(explanation);
        TextView value = new TextView(context);
        value.setPadding(0, dp(12), 0, 0);
        value.setTextSize(20);
        value.setText((audio != null && session != null ? session.appliedMicBoost() : micBoost) * 10 + " dB");
        content.addView(value);

        SeekBar slider = new SeekBar(context);
        slider.setContentDescription(tr("鸭子收音强度"));
        slider.setMax(3);
        slider.setProgress(audio != null && session != null ? session.appliedMicBoost() : micBoost);
        slider.setKeyProgressIncrement(1);
        slider.setMinimumHeight(dp(48));
        content.addView(slider, new LinearLayout.LayoutParams(-1, -2));
        slider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar bar, int position, boolean fromUser) {
                if (!fromUser) return;
                value.setText(position * 10 + " dB");
                VoiceConnection current = voice;
                if (audio != null && (current == null || !current.setMicBoost(position))) {
                    showMicBoostFailure();
                    return;
                }
                micBoost = position;
                getPreferences(MODE_PRIVATE).edit().putInt("mic-boost", micBoost).apply();
            }

            @Override public void onStartTrackingTouch(SeekBar bar) {}
            @Override public void onStopTrackingTouch(SeekBar bar) {}
        });
        new AlertDialog.Builder(context).setTitle(tr("鸭子收音"))
                .setView(content).setPositiveButton(tr("完成"), null).show();
    }

    private void showMicBoostFailure() {
        Toast.makeText(this, tr("鸭子未能切换麦克风增益"), Toast.LENGTH_SHORT).show();
    }

    private void showLandscapeSettings(View anchor) {
        PopupMenu options = new PopupMenu(themedContext(), anchor);
        options.getMenu().add(tr("连接地址")).setOnMenuItemClickListener(item -> {
            showAddressDialog();
            return true;
        });
        options.getMenu().add(tr("检查更新")).setOnMenuItemClickListener(item -> {
            checkForUpdate();
            return true;
        });
        options.getMenu().add(tr("切回竖屏")).setOnMenuItemClickListener(item -> {
            setRequestedOrientation(ActivityInfo.SCREEN_ORIENTATION_USER_PORTRAIT);
            return true;
        });
        options.getMenu().add(light() ? tr("暗色") : tr("亮色")).setOnMenuItemClickListener(item -> {
            setAppearance(english() ? "en" : "zh-CN", light() ? "night" : "day", true);
            return true;
        });
        options.getMenu().add(english() ? "中文" : "English").setOnMenuItemClickListener(item -> {
            setAppearance(english() ? "zh-CN" : "en", light() ? "day" : "night", true);
            return true;
        });
        options.show();
    }

    private void updateViewport() {
        boolean overlay = isLandscape() && overlaySupported;
        int standardButtons = overlay ? View.GONE : View.VISIBLE;
        addressButton.setVisibility(standardButtons);
        updateButton.setVisibility(standardButtons);
        orientationButton.setVisibility(standardButtons);
        landscapeSettings.setVisibility(overlay ? View.VISIBLE : View.GONE);
        addressLabel.setMaxWidth(overlay ? dp(132) : Integer.MAX_VALUE);
        int headerHeight = appHeader.getHeight();
        int footerHeight = voiceFooter.getHeight();
        FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) web.getLayoutParams();
        int top = overlay ? 0 : headerHeight;
        int bottom = overlay ? 0 : footerHeight;
        if (params.topMargin != top || params.bottomMargin != bottom) {
            params.topMargin = top;
            params.bottomMargin = bottom;
            web.setLayoutParams(params);
        }
        appHeader.setBackgroundColor(overlay ? Color.TRANSPARENT : surfaceColor());
        voiceFooter.setBackgroundColor(overlay ? Color.TRANSPARENT : surfaceColor());
        if (overlaySupported) {
            float density = getResources().getDisplayMetrics().density;
            int topCss = overlay ? (int) Math.ceil(headerHeight / density) : 0;
            int bottomCss = overlay ? (int) Math.ceil(footerHeight / density) : 0;
            web.evaluateJavascript("document.documentElement.style.setProperty('--app-native-top','"
                    + topCss + "px');document.documentElement.style.setProperty('--app-native-bottom','"
                    + bottomCss + "px');document.documentElement.classList.toggle('merged-toolbar',"
                    + overlay + ");", null);
        }
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        activePointer = -1;
        VoiceAudio session = audio;
        if (session != null) session.endTalk();
        web.evaluateJavascript("window.dispatchEvent(new Event('blur'));", null);
        updateOrientationLayout();
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
        label(voiceStatus, "正在打开麦克风");
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

    private void checkForUpdate() {
        EditText input = new EditText(themedContext());
        input.setInputType(InputType.TYPE_CLASS_PHONE);
        input.setSingleLine();
        input.setHint(tr("测试电脑或板端的局域网 IP"));
        input.setText(getPreferences(MODE_PRIVATE).getString("updateHost", ""));
        AlertDialog dialog = new AlertDialog.Builder(themedContext()).setTitle(tr("局域网检查更新"))
                .setMessage(tr("手机和更新服务器连接同一 Wi-Fi，端口 " + AppUpdates.PORT))
                .setView(input).setNegativeButton(tr("取消"), null).setPositiveButton(tr("检查"), null).create();
        dialog.setOnShowListener(event -> dialog.getButton(AlertDialog.BUTTON_POSITIVE)
                .setOnClickListener(view -> {
                    String source = input.getText().toString().trim();
                    if (!isLocalIp(source)) {
                        input.setError(tr("请输入有效的局域网 IP"));
                        return;
                    }
                    getPreferences(MODE_PRIVATE).edit().putString("updateHost", source).apply();
                    dialog.dismiss();
                    queryUpdate(source);
                }));
        dialog.show();
    }

    private void setUpdateBusy(String label) {
        updateButton.setEnabled(label == null);
        label(updateButton, label == null ? "检查更新" : label);
    }

    private void queryUpdate(String source) {
        setUpdateBusy("检查中");
        updateCall = AppUpdates.check(this, localWifi(source), source, (release, failure) -> runOnUiThread(() -> {
            if (isDestroyed() || isFinishing()) return;
            setUpdateBusy(null);
            if (!foreground) return;
            if (failure != null) {
                Toast.makeText(this, tr(failure), Toast.LENGTH_LONG).show();
                return;
            }
            PackageInfo installed;
            try {
                installed = getPackageManager().getPackageInfo(getPackageName(), 0);
            } catch (PackageManager.NameNotFoundException error) {
                Toast.makeText(this, tr("无法读取当前 App 版本"), Toast.LENGTH_SHORT).show();
                return;
            }
            if (release.code <= AppUpdates.versionCode(installed)) {
                Toast.makeText(this, tr("没有更新，当前版本 " + installed.versionName), Toast.LENGTH_LONG).show();
                return;
            }
            new AlertDialog.Builder(themedContext()).setTitle(tr("发现新版本 " + release.version))
                    .setMessage(String.format(tr("当前版本 %s。下载后按手机提示覆盖安装。"),
                            installed.versionName))
                    .setNegativeButton(tr("稍后"), null)
                    .setPositiveButton(tr("下载 APK"), (dialog, which) -> downloadUpdate(source, release))
                    .show();
        }));
    }

    private void downloadUpdate(String source, AppUpdates.Release release) {
        setUpdateBusy("下载中");
        updateCall = AppUpdates.download(this, localWifi(source), source, release, (apk, failure) ->
                runOnUiThread(() -> {
                    if (isDestroyed() || isFinishing()) return;
                    setUpdateBusy(null);
                    if (failure != null) {
                        if (foreground) Toast.makeText(this, tr(failure), Toast.LENGTH_LONG).show();
                        return;
                    }
                    installPending = true;
                    if (foreground) showInstallPrompt();
                }));
    }

    private void showInstallPrompt() {
        installPending = false;
        new AlertDialog.Builder(themedContext()).setTitle(tr("安装包已下载"))
                .setMessage(tr("按 Android 提示覆盖安装，原来的鸭子地址会保留。"))
                .setNegativeButton(tr("稍后"), null)
                .setPositiveButton(tr("安装"), (dialog, which) -> installUpdate())
                .show();
    }

    private void installUpdate() {
        try {
            if (!getPackageManager().canRequestPackageInstalls()) {
                new AlertDialog.Builder(themedContext()).setTitle(tr("允许 EasyDuck 安装测试包"))
                        .setMessage(tr("在系统设置里打开「允许来自此来源」，返回后继续安装。"))
                        .setNegativeButton(tr("取消"), null)
                        .setPositiveButton(tr("去设置"), (dialog, which) -> {
                            try {
                                startActivityForResult(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                                        Uri.parse("package:" + getPackageName())), INSTALL_PERMISSION);
                            } catch (ActivityNotFoundException error) {
                                Toast.makeText(this, tr("找不到系统安装权限设置"), Toast.LENGTH_LONG).show();
                            }
                        }).show();
                return;
            }
            if (!AppUpdates.downloadedApk(this).isFile()) {
                Toast.makeText(this, tr("安装包已被清理，请重新下载"), Toast.LENGTH_LONG).show();
                return;
            }
            Uri uri = Uri.parse("content://" + getPackageName() + ".updates/" + AppUpdates.APK_NAME);
            startActivity(new Intent(Intent.ACTION_VIEW).setDataAndType(uri, UpdateApkProvider.APK_TYPE)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION));
        } catch (ActivityNotFoundException | SecurityException error) {
            Toast.makeText(this, tr("无法打开 Android 安装界面"), Toast.LENGTH_LONG).show();
        }
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == INSTALL_PERMISSION && getPackageManager().canRequestPackageInstalls()) {
            installPending = true;
        }
    }

    private void showAddressDialog() {
        EditText input = new EditText(themedContext());
        input.setInputType(InputType.TYPE_CLASS_PHONE);
        input.setSingleLine();
        input.setText(host == null ? "" : host);
        input.setHint(tr("例如 192.168.1.100"));
        AlertDialog dialog = new AlertDialog.Builder(themedContext()).setTitle(tr("鸭子的局域网 IP"))
                .setView(input).setNegativeButton(tr("取消"), null)
                .setPositiveButton(tr("连接"), null).create();
        dialog.setOnShowListener(event -> dialog.getButton(AlertDialog.BUTTON_POSITIVE)
                .setOnClickListener(view -> {
                    String candidate = input.getText().toString().trim();
                    if (!isLocalIp(candidate)) {
                        input.setError(tr("请输入有效的局域网 IP"));
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
        label(addressLabel, "无法连到鸭子，请核对 Wi-Fi 和地址");
    }

    private void openController() {
        bindLocalWifi();
        controlFailed = false;
        label(addressLabel, "正在连接 " + host);
        Uri.Builder address = new Uri.Builder().scheme("http").encodedAuthority(host + ":9527")
                .path("/").appendQueryParameter("view", "app").appendQueryParameter("nativeUi", "1");
        if (language != null) address.appendQueryParameter("lang", language);
        if (theme != null) address.appendQueryParameter("theme", theme);
        web.loadUrl(address.build().toString());
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
        label(voiceStatus, "正在连接对讲");
        label(voiceButton, "取消");
        try {
            VoiceConnection.Listener listener = new VoiceConnection.Listener() {
                @Override public void micBoostFailed() {
                    runOnUiThread(() -> {
                        if (this == voiceListener) showMicBoostFailure();
                    });
                }
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
            voice = new VoiceConnection(address, micBoost, listener);
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
                        label(voiceStatus, "正在收听鸭子周围的声音");
                        label(talkButton, "按住说话");
                    } });
                }
                @Override public void focusPaused() {
                    runOnUiThread(() -> {
                        if (owner != voiceListener || audio == null || !audio.isFocusPaused()) return;
                        activePointer = -1;
                        label(voiceStatus, "手机音频暂被占用，对讲保持连接");
                        label(talkButton, "恢复收听");
                    });
                }

                @Override public void speaking() {
                    runOnUiThread(() -> { if (owner == voiceListener && audio != null && audio.isTalking()) {
                        label(voiceStatus, "正在说话");
                        label(talkButton, "松开收听");
                    } });
                }
                @Override public void route(boolean speaker) {
                    runOnUiThread(() -> { if (owner == voiceListener) {
                        label(routeStatus, speaker ? "手机外放" : "手机未使用外放");
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
            if (voice.supportsMicBoost() && voice.appliedMicBoost() != micBoost) voice.setMicBoost(micBoost);
            setVolumeControlStream(AudioManager.STREAM_VOICE_CALL);
            label(voiceStatus, "正在收听鸭子周围的声音");
            voiceWasReady = true;
            voiceRetries = 0;
            label(voiceButton, "关闭对讲");
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
        label(voiceButton, "取消重连");
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
        label(voiceButton, "开启对讲");
        label(talkButton, "按住说话");
        talkButton.setEnabled(false);
        label(voiceStatus, reason);
        label(routeStatus, "");
        setVolumeControlStream(AudioManager.USE_DEFAULT_STREAM_TYPE);
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request != MICROPHONE_PERMISSION) return;
        if (results.length == 0 || results[0] != PackageManager.PERMISSION_GRANTED) {
            label(voiceStatus, "开启对讲需要麦克风权限");
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
        if (installPending) showInstallPrompt();
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
        if (updateCall != null) updateCall.cancel();
        if (speakerVolumeCall != null) speakerVolumeCall.cancel();
        getSystemService(ConnectivityManager.class).bindProcessToNetwork(null);
        web.destroy();
        super.onDestroy();
    }
}
