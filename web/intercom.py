"""One authenticated, half-duplex PCM stream for the local web controller."""
import hmac
import math
import os
import secrets
import select
import subprocess
import threading
import time

PCM = ('-t', 'raw', '-f', 'S16_LE', '-r', '16000', '-c', '1',
       '--period-time=20000', '--buffer-time=80000')
PACKET = 1920  # 60 ms, signed 16-bit little-endian mono.
CAPTURE = 'plughw:CARD=rockchiprk809co,DEV=0'
MIC_BOOST = 'MIC Boost Gain'
SPEAKER_VOLUME = 'DAC Playback Volume'
SPEAKER_DEFAULT = 252
SPEAKER_STEP = 5
DAC_DB_PER_STEP = (95.00 - 1.12) / 255


class IntercomError(Exception):
    def __init__(self, status, detail):
        super().__init__(detail)
        self.status = status


class Intercom:
    def __init__(self, auth, *, mic_boost=1, session_timeout=4):
        self.auth = auth
        self.mic_boost = mic_boost
        self.session_timeout = session_timeout
        self.lock = threading.Lock()
        self.speaker_lock = threading.Lock()
        self.token = None
        self.stream_token = None
        self.expires = 0
        self.next_login = 0
        self.process = None
        self.mode = None
        self.mixer_values = {}
        self.received = bytearray()
        self.stop = threading.Event()
        self.watchdog = threading.Thread(target=self._expire, daemon=True)
        self.watchdog.start()

    def _expire(self):
        while not self.stop.wait(0.4):
            with self.lock:
                if self.token and time.monotonic() >= self.expires:
                    self._reset()

    def _mixer(self, action, name, *values):
        return subprocess.check_output(
            ('amixer', '-c', 'rockchiprk809co', action, f'name={name}', *values),
            stderr=subprocess.DEVNULL, timeout=1, text=True,
        )

    def _set_mixer(self, name, value):
        current = self._mixer('cget', name).rsplit(': values=', 1)[-1].split()[0]
        self.mixer_values[name] = current
        self._mixer('cset', name, value)

    def speaker_volume(self, percent=None):
        with self.speaker_lock:
            try:
                if percent is not None:
                    raw = (0 if percent == 0 else
                           SPEAKER_DEFAULT + round(20 * math.log10(percent / 100) / DAC_DB_PER_STEP))
                    self._mixer('cset', SPEAKER_VOLUME, f'{raw},{raw}')
                    return percent
                result = self._mixer('cget', SPEAKER_VOLUME)
                raw = int(result.rsplit(': values=', 1)[1].split(',')[0])
                if raw == 0:
                    return 0
                percent = 100 * 10 ** ((raw - SPEAKER_DEFAULT) * DAC_DB_PER_STEP / 20)
                return min(100, max(SPEAKER_STEP, round(percent / SPEAKER_STEP) * SPEAKER_STEP))
            except (ValueError, IndexError, OSError, subprocess.SubprocessError) as error:
                raise IntercomError(503, 'intercom_audio_failed') from error

    def _stop_process(self, drain=False):
        process, self.process = self.process, None
        self.mode = None
        self.received.clear()
        if process is None:
            return
        if process.stdin:
            process.stdin.close()
        if drain and process.poll() is None:
            try:
                process.wait(timeout=0.2)
            except subprocess.TimeoutExpired:
                pass
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=0.2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if process.stdout:
            process.stdout.close()

    def _reset(self):
        self.token = None
        self._stop_process()
        for name, value in reversed(tuple(self.mixer_values.items())):
            try:
                self._mixer('cset', name, value)
            except (OSError, subprocess.SubprocessError):
                print(f'Intercom could not restore {name}', flush=True)
        self.mixer_values.clear()

    def shutdown(self):
        self.stop.set()
        with self.lock:
            self._reset()

    def open(self, username=None, password=None, *, mic_boost=None):
        gain = self.mic_boost if mic_boost is None else mic_boost
        if type(gain) is not int or not 0 <= gain <= 3:
            raise IntercomError(400, 'intercom_invalid_mic_boost')
        with self.lock:
            now = time.monotonic()
            if now < self.next_login:
                raise IntercomError(429, 'intercom_retry')
            self.next_login = now + 1
        if self.auth is not None and not self.auth.verify(username, password):
            raise IntercomError(401, 'intercom_login_failed')
        with self.lock:
            if self.token and time.monotonic() < self.expires:
                raise IntercomError(409, 'intercom_busy')
            self._reset()
            try:
                self._set_mixer('Capture MIC Path', 'Main Mic')
                self._set_mixer(MIC_BOOST, f'{gain},{gain}')
            except (OSError, subprocess.SubprocessError, IndexError):
                self._reset()
                raise IntercomError(503, 'intercom_audio_failed')
            self.token = secrets.token_hex(24)
            self.expires = time.monotonic() + 4
            return self.token

    def set_mic_boost(self, token, gain):
        if type(gain) is not int or not 0 <= gain <= 3:
            raise IntercomError(400, 'intercom_invalid_mic_boost')
        with self.lock:
            if (not self.token or self.stream_token != token or time.monotonic() >= self.expires
                    or not hmac.compare_digest(self.token, token)):
                raise IntercomError(401, 'intercom_closed')
            # Keep the original mixer value saved by open() for session cleanup.
            try:
                self._mixer('cset', MIC_BOOST, f'{gain},{gain}')
            except (OSError, subprocess.SubprocessError) as error:
                raise IntercomError(503, 'intercom_audio_failed') from error

    def _switch(self, mode):
        if self.mode == mode and self.process.poll() is None:
            return
        self._stop_process(drain=mode == 'listen')
        if mode == 'listen':
            args = ('arecord', '-q', '-D', CAPTURE, *PCM)
            self.process = subprocess.Popen(
                args, stdout=subprocess.PIPE, stdin=subprocess.DEVNULL, bufsize=0)
        else:
            args = ('aplay', '-q', '-D', 'easyduck_speaker', *PCM)
            self.process = subprocess.Popen(
                args, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, bufsize=0)
        stream = self.process.stdout if mode == 'listen' else self.process.stdin
        os.set_blocking(stream.fileno(), False)
        self.mode = mode

    def claim_stream(self, token):
        with self.lock:
            if (self.stream_token or not self.token or time.monotonic() >= self.expires
                    or not hmac.compare_digest(self.token, token)):
                return False
            self.stream_token = token
            self.expires = time.monotonic() + self.session_timeout
            return True

    def release_stream(self, token):
        with self.lock:
            if self.stream_token == token:
                self.stream_token = None
                if self.token == token:
                    self._reset()

    def exchange(self, token, mode, data):
        with self.lock:
            if not self.token or not token.isascii() or not hmac.compare_digest(self.token, token):
                raise IntercomError(401, 'intercom_closed')
            if mode == 'close' or time.monotonic() >= self.expires:
                self._reset()
                if mode == 'close':
                    return b''
                raise IntercomError(401, 'intercom_closed')
            self.expires = time.monotonic() + self.session_timeout
            try:
                self._switch(mode)
                if mode == 'listen':
                    return self._record()
                if data:
                    fd = self.process.stdin.fileno()
                    if select.select([], [fd], [], 0.12)[1]:
                        os.write(fd, data)
                return b''
            except (OSError, subprocess.SubprocessError):
                self._reset()
                raise IntercomError(503, 'intercom_audio_failed')

    def _record(self):
        fd = self.process.stdout.fileno()
        deadline = time.monotonic() + 0.25
        while len(self.received) < PACKET:
            remaining = max(0, deadline - time.monotonic())
            if not select.select([fd], [], [], remaining)[0]:
                return b''
            data = os.read(fd, PACKET - len(self.received))
            if not data:
                raise OSError('ALSA capture closed')
            self.received.extend(data)
        data = bytes(self.received)
        self.received.clear()
        return data
