package com.lckfb.easyduck;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.net.Network;
import android.os.Build;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.Proxy;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Locale;
import java.util.concurrent.TimeUnit;

import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.ResponseBody;
import okio.ByteString;
import org.json.JSONObject;

final class AppUpdates {
    static final int PORT = 8766;
    static final String APK_NAME = "EasyDuck-update.apk";
    private static final long MAX_APK_BYTES = 50 * 1024 * 1024;
    private static final long MAX_MANIFEST_BYTES = 4096;
    private static final OkHttpClient CLIENT = new OkHttpClient.Builder()
            .proxy(Proxy.NO_PROXY).followRedirects(false).followSslRedirects(false)
            .connectTimeout(5, TimeUnit.SECONDS).readTimeout(10, TimeUnit.SECONDS)
            .callTimeout(120, TimeUnit.SECONDS).build();

    static final class Release {
        final int code;
        final String version;
        final long bytes;
        final String sha256;

        Release(int code, String version, long bytes, String sha256) {
            this.code = code;
            this.version = version;
            this.bytes = bytes;
            this.sha256 = sha256;
        }
    }

    interface Result<T> {
        void done(T value, String error);
    }

    static File downloadedApk(Context context) {
        return new File(context.getCacheDir(), APK_NAME);
    }

    static Call check(Context context, Network network, String host, Result<Release> result) {
        return fetch(network, host, "update.json", body -> {
            if (body.source().request(MAX_MANIFEST_BYTES + 1)) throw new IOException();
            JSONObject data = new JSONObject(body.string());
            int code = data.getInt("versionCode");
            String version = data.getString("versionName");
            long bytes = data.getLong("size");
            String sha256 = data.getString("sha256").toLowerCase(Locale.ROOT);
            if (!context.getPackageName().equals(data.getString("packageName"))
                    || code <= 0 || !version.matches("[0-9]{1,6}(\\.[0-9]{1,6}){2}")
                    || bytes <= 0 || bytes > MAX_APK_BYTES || !sha256.matches("[0-9a-f]{64}")) {
                throw new IOException();
            }
            return new Release(code, version, bytes, sha256);
        }, "无法读取更新信息，请检查局域网地址和 update.json", result);
    }

    static Call download(Context context, Network network, String host, Release release, Result<File> result) {
        return fetch(network, host, APK_NAME, body -> {
            File partial = File.createTempFile("easyduck-", ".apk", context.getCacheDir());
            try {
                MessageDigest sha256 = MessageDigest.getInstance("SHA-256");
                long received = 0;
                try (InputStream input = body.byteStream();
                     FileOutputStream output = new FileOutputStream(partial)) {
                    byte[] buffer = new byte[16384];
                    int count;
                    while ((count = input.read(buffer)) != -1) {
                        received += count;
                        if (received > release.bytes) throw new IOException();
                        sha256.update(buffer, 0, count);
                        output.write(buffer, 0, count);
                    }
                }
                if (received != release.bytes
                        || !release.sha256.equals(ByteString.of(sha256.digest()).hex())
                        || !validApk(context, partial, release)) {
                    throw new IOException();
                }
                File apk = downloadedApk(context);
                Files.move(partial.toPath(), apk.toPath(), StandardCopyOption.REPLACE_EXISTING,
                        StandardCopyOption.ATOMIC_MOVE);
                return apk;
            } finally {
                partial.delete();
            }
        }, "APK 下载或校验失败，请检查安装包、版本和签名", result);
    }

    @SuppressWarnings("deprecation")
    static long versionCode(PackageInfo info) {
        return Build.VERSION.SDK_INT >= 28 ? info.getLongVersionCode() : info.versionCode;
    }

    @SuppressWarnings("deprecation")
    private static boolean validApk(Context context, File file, Release release)
            throws PackageManager.NameNotFoundException {
        PackageManager manager = context.getPackageManager();
        int flags = Build.VERSION.SDK_INT >= 28
                ? PackageManager.GET_SIGNING_CERTIFICATES : PackageManager.GET_SIGNATURES;
        PackageInfo incoming = manager.getPackageArchiveInfo(file.getAbsolutePath(), flags);
        PackageInfo installed = manager.getPackageInfo(context.getPackageName(), flags);
        if (incoming == null || !installed.packageName.equals(incoming.packageName)
                || versionCode(incoming) != release.code || versionCode(incoming) <= versionCode(installed)
                || !release.version.equals(incoming.versionName)) return false;
        Signature[] expected = signers(installed);
        Signature[] actual = signers(incoming);
        return expected != null && expected.length > 0 && actual != null
                && new HashSet<>(Arrays.asList(expected)).equals(new HashSet<>(Arrays.asList(actual)));
    }

    @SuppressWarnings("deprecation")
    private static Signature[] signers(PackageInfo info) {
        if (Build.VERSION.SDK_INT < 28) return info.signatures;
        return info.signingInfo == null ? null : info.signingInfo.getApkContentsSigners();
    }

    private interface ReadBody<T> {
        T read(ResponseBody body) throws IOException, org.json.JSONException,
                NoSuchAlgorithmException, PackageManager.NameNotFoundException;
    }

    private static <T> Call fetch(Network network, String host, String file, ReadBody<T> read,
            String message, Result<T> result) {
        HttpUrl url = new HttpUrl.Builder().scheme("http").host(host).port(PORT)
                .addPathSegment(file).build();
        OkHttpClient client = network == null ? CLIENT
                : CLIENT.newBuilder().socketFactory(network.getSocketFactory()).build();
        Call call = client.newCall(new Request.Builder().url(url).build());
        call.enqueue(new Callback() {
            @Override public void onFailure(Call call, IOException error) {
                result.done(null, message);
            }

            @Override public void onResponse(Call call, Response response) {
                T value;
                try (response) {
                    if (!response.isSuccessful() || response.body() == null) throw new IOException();
                    value = read.read(response.body());
                } catch (IOException | org.json.JSONException | NoSuchAlgorithmException
                        | PackageManager.NameNotFoundException error) {
                    result.done(null, message);
                    return;
                }
                result.done(value, null);
            }
        });
        return call;
    }
}
