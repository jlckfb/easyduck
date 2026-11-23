package com.lckfb.easyduck;

import java.io.IOException;
import java.net.Proxy;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;
import org.json.JSONObject;

final class VoiceConnection implements AutoCloseable {
    static final int PACKET_BYTES = 1920;
    private static final RequestBody EMPTY = RequestBody.create(new byte[0], null);

    interface Listener {
        void ready();
        void audio(byte[] pcm);
        void failed(String reason);
    }

    private static final OkHttpClient CLIENT = new OkHttpClient.Builder()
            .proxy(Proxy.NO_PROXY)
            .followRedirects(false)
            .followSslRedirects(false)
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(5, TimeUnit.SECONDS)
            .build();
    private final HttpUrl base;
    private final String origin;
    private final Listener listener;
    private final ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor();
    private boolean closed;
    private boolean ready;
    private WebSocket socket;
    private Call login;
    private String token;

    VoiceConnection(String host, Listener listener) {
        this.listener = listener;
        base = new HttpUrl.Builder().scheme("http").host(host).port(9460).build();
        origin = "http://" + host + ":9460";
    }

    private Request.Builder request(String path) {
        return request(base, origin, path);
    }

    private static Request.Builder request(HttpUrl url, String origin, String path) {
        return new Request.Builder().url(url.newBuilder().addPathSegments("api/intercom/" + path).build())
                .header("Origin", origin);
    }

    synchronized void open() {
        if (closed || login != null) return;
        login = CLIENT.newCall(request("open").post(EMPTY).build());
        login.enqueue(new Callback() {
            @Override public void onFailure(Call call, IOException error) {
                fail("无法连接鸭子的对讲服务，请检查 Wi-Fi 和鸭子的 IP");
            }

            @Override public void onResponse(Call call, Response response) {
                try (response) {
                    if (!response.isSuccessful() || response.body() == null) {
                        String reason = switch (response.code()) {
                            case 409 -> "另一台手机正在使用对讲";
                            case 429 -> "请稍等一秒后重试";
                            default -> "鸭子的对讲服务暂时不可用";
                        };
                        fail(reason);
                        return;
                    }
                    JSONObject session = new JSONObject(response.body().string());
                    String received = session.getString("token");
                    if (!received.matches("[0-9a-f]{48}")) throw new IOException("Invalid voice session");
                    connect(received);
                } catch (IOException | org.json.JSONException error) {
                    fail("鸭子的对讲响应有误");
                }
            }
        });
    }

    private synchronized void connect(String received) {
        if (closed) {
            closeSession(received);
            return;
        }
        token = received;
        socket = CLIENT.newWebSocket(request("stream").build(), new WebSocketListener() {
            @Override public void onOpen(WebSocket webSocket, Response response) {
                if (!webSocket.send(received)) fail("无法开启对讲");
            }

            @Override public void onMessage(WebSocket webSocket, String text) {
                synchronized (VoiceConnection.this) {
                    if (closed || webSocket != socket) return;
                    if (ready || !"ready".equals(text)) return;
                    ready = true;
                }
                listener.ready();
            }

            @Override public void onMessage(WebSocket webSocket, ByteString bytes) {
                if (bytes.size() != PACKET_BYTES) return;
                synchronized (VoiceConnection.this) {
                    if (closed || !ready || webSocket != socket) return;
                }
                listener.audio(bytes.toByteArray());
            }

            @Override public void onClosing(WebSocket webSocket, int code, String reason) {
                webSocket.close(1000, null);
                fail("对讲已断开，请重新开启");
            }

            @Override public void onFailure(WebSocket webSocket, Throwable error, Response response) {
                fail("对讲已断开，请检查同一个 Wi-Fi 后重试");
            }
        });
        timer.scheduleAtFixedRate(() -> send("ping"), 1, 1, TimeUnit.SECONDS);
        timer.schedule(() -> {
            synchronized (this) {
                if (ready || closed) return;
            }
            fail("对讲连接超时");
        }, 5, TimeUnit.SECONDS);
    }

    synchronized boolean send(String mode) {
        return !closed && ready && socket != null && socket.send(mode);
    }

    synchronized boolean sendAudio(byte[] data, int length) {
        return !closed && ready && socket != null
                && socket.queueSize() < 12L * PACKET_BYTES
                && socket.send(ByteString.of(data, 0, length));
    }

    private void fail(String reason) {
        synchronized (this) {
            if (closed) return;
            close("connection_error");
        }
        listener.failed(reason);
    }

    private void closeSession(String session) {
        CLIENT.newCall(request("close").header("X-Intercom-Token", session).post(EMPTY).build())
                .enqueue(new Callback() {
                    @Override public void onFailure(Call call, IOException error) {}
                    @Override public void onResponse(Call call, Response response) {
                        response.close();
                    }
                });
    }

    @Override public void close() {
        close(null);
    }

    synchronized void close(String reason) {
        if (closed) return;
        closed = true;
        timer.shutdownNow();
        if (login != null) login.cancel();
        if (socket != null) {
            if (ready) socket.close(1000, reason);
            else socket.cancel();
        }
        if (!ready && token != null) closeSession(token);
    }
}
