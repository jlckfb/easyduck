package com.lckfb.easyduck;

import android.content.Context;
import android.media.AudioAttributes;
import android.media.AudioDeviceInfo;
import android.media.AudioFocusRequest;
import android.media.AudioFormat;
import android.media.AudioManager;
import android.media.AudioRecord;
import android.media.AudioTrack;
import android.media.MediaRecorder;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;

final class VoiceAudio implements AutoCloseable {
    private static final int RATE = 16000;
    private static final int FRAME = VoiceConnection.PACKET_BYTES;
    private static final AudioAttributes ATTRIBUTES = new AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build();

    interface Listener {
        void listening();
        void speaking();
        void focusPaused();
        void route(boolean speaker);
        void failed(String message, String code);
    }

    private final AudioManager manager;
    private final AudioFocusRequest focus;
    private final VoiceConnection connection;
    private final Listener listener;
    private final ArrayBlockingQueue<byte[]> queue = new ArrayBlockingQueue<>(6);
    private final int priorMode;
    private final boolean priorSpeaker;
    private final AudioTrack player;
    private volatile boolean closed;
    private volatile boolean talking;
    private volatile boolean talkRequested;
    private volatile boolean captureActive;
    private volatile boolean focusPaused;
    private AudioRecord recorder;

    @SuppressWarnings("deprecation")
    VoiceAudio(Context context, VoiceConnection connection, Listener listener) {
        this.connection = connection;
        this.listener = listener;
        manager = context.getSystemService(AudioManager.class);
        priorMode = manager.getMode();
        priorSpeaker = manager.isSpeakerphoneOn();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
                .setAudioAttributes(ATTRIBUTES)
                .setWillPauseWhenDucked(true)
                .setOnAudioFocusChangeListener(this::onFocusChange, new Handler(Looper.getMainLooper()))
                .build();
        if (manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
            throw new IllegalStateException("无法取得手机音频");
        }

        AudioTrack output = null;
        try {
            manager.setMode(AudioManager.MODE_IN_COMMUNICATION);
            AudioDeviceInfo speaker = requestSpeaker();
            int minimum = AudioTrack.getMinBufferSize(
                    RATE, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT);
            if (minimum < 0) throw new IllegalStateException("手机扬声器不支持对讲采样率");
            output = new AudioTrack.Builder().setAudioAttributes(ATTRIBUTES)
                    .setAudioFormat(format(AudioFormat.CHANNEL_OUT_MONO))
                    .setTransferMode(AudioTrack.MODE_STREAM)
                    .setBufferSizeInBytes(Math.max(minimum, FRAME * 3)).build();
            if (output.getState() != AudioTrack.STATE_INITIALIZED) {
                throw new IllegalStateException("手机扬声器无法启动");
            }
            if (speaker != null) output.setPreferredDevice(speaker);
            output.addOnRoutingChangedListener(router -> {
                AudioDeviceInfo actual = router.getRoutedDevice();
                if (actual != null && !closed) {
                    listener.route(actual.getType() == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER);
                }
            }, new Handler(Looper.getMainLooper()));
            player = output;
        } catch (RuntimeException error) {
            if (output != null) output.release();
            restoreRoute();
            throw error;
        }
        new Thread(this::play, "EasyDuck playback").start();
    }

    private static AudioFormat format(int channel) {
        return new AudioFormat.Builder().setSampleRate(RATE).setChannelMask(channel)
                .setEncoding(AudioFormat.ENCODING_PCM_16BIT).build();
    }

    @SuppressWarnings("deprecation")
    private AudioDeviceInfo requestSpeaker() {
        if (Build.VERSION.SDK_INT >= 31) {
            for (AudioDeviceInfo target : manager.getAvailableCommunicationDevices()) {
                if (target.getType() == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER
                        && manager.setCommunicationDevice(target)) return target;
            }
        } else {
            manager.setSpeakerphoneOn(true);
        }
        for (AudioDeviceInfo target : manager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)) {
            if (target.getType() == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER) return target;
        }
        return null;
    }

    private synchronized void onFocusChange(int change) {
        if (closed) return;
        switch (change) {
            case AudioManager.AUDIOFOCUS_LOSS:
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK:
                focusPaused = true;
                endTalk();
                queue.clear();
                listener.focusPaused();
                break;
            case AudioManager.AUDIOFOCUS_GAIN:
                if (focusPaused) {
                    focusPaused = false;
                    listener.listening();
                }
                break;
            default:
                break;
        }
    }

    boolean isFocusPaused() { return focusPaused; }

    synchronized void resumeListening() {
        if (closed || !focusPaused) return;
        if (manager.requestAudioFocus(focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
            onFocusChange(AudioManager.AUDIOFOCUS_GAIN);
        }
    }

    void receive(byte[] frame) {
        if (closed || captureActive || focusPaused) return;
        if (!queue.offer(frame)) {
            queue.poll();
            queue.offer(frame);
        }
    }

    private void play() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_AUDIO);
        boolean running = false;
        int reportedDevice = -1;
        try {
            while (!closed) {
                byte[] frame = queue.poll(60, TimeUnit.MILLISECONDS);
                if (captureActive || focusPaused) {
                    if (running) {
                        player.pause();
                        player.flush();
                        running = false;
                    }
                    continue;
                }
                if (frame == null) continue;
                if (!running) {
                    player.play();
                    running = true;
                }
                int offset = 0;
                while (offset < frame.length && !closed && !captureActive && !focusPaused) {
                    int written = player.write(frame, offset, frame.length - offset, AudioTrack.WRITE_BLOCKING);
                    if (written <= 0) throw new IllegalStateException("手机扬声器播放失败");
                    offset += written;
                }
                AudioDeviceInfo device = player.getRoutedDevice();
                if (device != null && device.getId() != reportedDevice) {
                    reportedDevice = device.getId();
                    listener.route(device.getType() == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER);
                }
            }
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
        } catch (RuntimeException error) {
            if (!closed) listener.failed("手机扬声器播放失败", "playback_error");
        } finally {
            player.release();
        }
    }

    boolean isTalking() { return talkRequested; }

    synchronized void beginTalk() {
        if (closed || focusPaused) return;
        talkRequested = true;
        if (captureActive) return;
        if (!connection.send("talk")) {
            talkRequested = false;
            listener.failed("对讲连接已断开", "connection_error");
            return;
        }
        talking = true;
        captureActive = true;
        queue.clear();
        new Thread(this::record, "EasyDuck microphone").start();
    }

    synchronized void endTalk() {
        talkRequested = false;
        talking = false;
        if (recorder != null && recorder.getRecordingState() == AudioRecord.RECORDSTATE_RECORDING) {
            recorder.stop();
        }
    }

    private void record() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_AUDIO);
        AudioRecord input = null;
        byte[] frame = new byte[FRAME];
        int length = 0;
        try {
            synchronized (this) {
                if (!talking || closed) return;
                int minimum = AudioRecord.getMinBufferSize(
                        RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
                if (minimum < 0) throw new IllegalStateException("手机麦克风不支持对讲采样率");
                input = new AudioRecord.Builder()
                        .setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
                        .setAudioFormat(format(AudioFormat.CHANNEL_IN_MONO))
                        .setBufferSizeInBytes(Math.max(minimum, FRAME * 3)).build();
                recorder = input;
                if (input.getState() != AudioRecord.STATE_INITIALIZED) {
                    throw new IllegalStateException("手机麦克风无法启动");
                }
                input.startRecording();
                if (input.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) {
                    throw new IllegalStateException("手机麦克风无法开始录音");
                }
            }
            listener.speaking();
            while (talking && !closed) {
                int read = input.read(frame, length, frame.length - length, AudioRecord.READ_BLOCKING);
                if (read <= 0) {
                    if (talking && !closed) throw new IllegalStateException("手机麦克风读取失败");
                    break;
                }
                length += read;
                if (length == frame.length) {
                    if (!focusPaused) connection.sendAudio(frame, length);
                    length = 0;
                }
            }
            if (!closed && !focusPaused && length > 0) connection.sendAudio(frame, length - (length % 2));
        } catch (RuntimeException error) {
            synchronized (this) {
                if (!closed && talking) {
                    talkRequested = false;
                    listener.failed("手机麦克风无法录音，请检查权限", "recording_error");
                }
            }
        } finally {
            synchronized (this) {
                recorder = null;
                if (input != null) {
                    if (input.getRecordingState() == AudioRecord.RECORDSTATE_RECORDING) input.stop();
                    input.release();
                }
                talking = false;
                captureActive = false;
                if (!closed) {
                    if (talkRequested) beginTalk();
                    else {
                        connection.send("listen");
                        listener.listening();
                    }
                }
            }
        }
    }

    @SuppressWarnings("deprecation")
    private void restoreRoute() {
        if (Build.VERSION.SDK_INT >= 31) manager.clearCommunicationDevice();
        else manager.setSpeakerphoneOn(priorSpeaker);
        manager.setMode(priorMode);
        manager.abandonAudioFocusRequest(focus);
    }

    @Override public synchronized void close() {
        if (closed) return;
        closed = true;
        endTalk();
        queue.clear();
        restoreRoute();
    }
}
