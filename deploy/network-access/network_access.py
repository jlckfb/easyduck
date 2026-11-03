"""Offline Wi-Fi address announcement. Reads robot state; never sends motion requests."""
import argparse
import ipaddress
import json
import logging
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time
import tempfile
import wave
import tomllib

deploy = Path(__file__).resolve().parent.parent
if not (deploy / 'network_routes.py').is_file():
    deploy = deploy / 'deploy'
sys.path.insert(0, str(deploy))
from network_routes import addresses

LOG = logging.getLogger('easyduck-network')
DIGITS = ('ling2', 'yi1', 'er4', 'san1', 'si4', 'wu3', 'liu4', 'qi1', 'ba1', 'jiu3')


def speech_text(address):
    address = str(ipaddress.IPv4Address(address))
    # Explicit Pinyin avoids the cmn Han-character dictionary's English fallback.
    spoken = ' '.join('dian3' if c == '.' else DIGITS[int(c)] for c in address)
    return 'wang3 luo4 yi3 lian2 jie1. di4 zhi3. ' + spoken + '. wang3 ye4 duan1 kou3. jiu3 wu3 er4 qi1.'


def speech_segments(address):
    octets = str(ipaddress.IPv4Address(address)).split('.')
    parts = [('wang3 luo4 yi3 lian2 jie1', 800), ('di4 zhi3', 600)]
    for index, octet in enumerate(octets):
        parts.append((' '.join(DIGITS[int(c)] for c in octet), 500))
        if index < 3:
            parts.append(('dian3', 350))
    parts += [('wang3 ye4 duan1 kou3', 600), ('jiu3 wu3 er4 qi1', 500)]
    return parts


def render_speech(address, destination, voice='cmn-latn-pinyin'):
    # Insert actual silent PCM frames; punctuation alone has voice-dependent timing.
    parts = speech_segments(address)
    destination = Path(destination)
    with tempfile.TemporaryDirectory(prefix='speech-', dir=destination.parent) as folder:
        format = None
        with wave.open(str(destination), 'wb') as output:
            for index, (text, pause_ms) in enumerate(parts):
                part = Path(folder) / (str(index) + '.wav')
                subprocess.run(['espeak-ng', '-v', voice, '-s', '135', '-a', '65',
                                '-w', str(part), text], check=True, capture_output=True, timeout=10)
                with wave.open(str(part), 'rb') as source:
                    actual = (source.getnchannels(), source.getsampwidth(), source.getframerate())
                    if format is None:
                        format = actual
                        if format[1] != 2: raise ValueError('Expected signed 16-bit speech PCM')
                        output.setnchannels(format[0]); output.setsampwidth(format[1]); output.setframerate(format[2])
                    if actual != format: raise ValueError('Inconsistent speech PCM format')
                    output.writeframes(source.readframes(source.getnframes()))
                silence = round(format[2] * pause_ms / 1000)
                output.writeframes(b'\0' * silence * format[0] * format[1])


def mdns_name():
    try:
        result = subprocess.run(['busctl', '--system', 'call', 'org.freedesktop.Avahi', '/',
                                 'org.freedesktop.Avahi.Server', 'GetHostNameFqdn'],
                                check=True, capture_output=True, text=True, timeout=2)
        kind, value = result.stdout.strip().split(' ', 1)
        name = json.loads(value)
        if kind == 's' and isinstance(name, str) and name.endswith('.local') and all(
                c.isascii() and (c.isalnum() or c in '.-') for c in name):
            return name
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    return None


def robot_idle(path='/run/robotd.sock'):
    # An absent daemon cannot own audio. An unreadable/running daemon is not assumed idle.
    if not Path(path).exists():
        return True
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(0.6)
            s.connect(path)
            s.sendall((json.dumps({'jsonrpc':'2.0', 'id':1, 'method':'robot.subscribe',
                                  'params':{'hz':1}}) + '\n').encode())
            with s.makefile('rb') as stream:
                for _ in range(3):
                    message = json.loads(stream.readline(1048576))
                    if message.get('method') == 'robot.state':
                        state = message.get('params', {})
                        if isinstance(state.get('enabled'), bool):
                            return not state['enabled']
                        return state.get('policy') == 'xgo_off'
    except (OSError, ValueError):
        pass
    return False


def pcm_idle():
    try:
        # Defer to greeting, low-battery warning and other existing sound playback.
        return all(p.read_text().strip() == 'closed' for p in
                   Path('/proc/asound').glob('card*/pcm*p/sub*/status'))
    except OSError:
        return False


class Announcer:
    def __init__(self, speak, clock=time.monotonic, stable_seconds=3, startup_seconds=15):
        self.speak, self.clock = speak, clock
        self.stable_seconds = stable_seconds
        self.started = self.changed = clock()
        self.startup_seconds = startup_seconds
        self.address, self.announced = '', ''
        self.retry_at = 0

    def tick(self, address, idle=True):
        now = self.clock()
        if address != self.address:
            self.address, self.changed, self.retry_at = address, now, 0
        if not address:
            if now - self.changed >= 10:
                self.announced = ''
            return False
        if (not idle or address == self.announced or now < self.retry_at
                or now - self.changed < self.stable_seconds
                or now - self.started < self.startup_seconds):
            return False
        self.retry_at = now + 15
        if self.speak(address):
            self.announced = address
            return True
        return False


class Speaker:
    def __init__(self, directory, voice='cmn-latn-pinyin', device='plughw:0,0'):
        self.directory, self.voice, self.device = Path(directory), voice, device
        self.last_error = None

    def __call__(self, address):
        try:
            self.directory.mkdir(parents=True, exist_ok=True)
            wav = self.directory / 'address.wav'
            render_speech(address, wav, self.voice)
            if not robot_idle() or not pcm_idle():
                return False
            player = subprocess.Popen(['aplay', '-q', '-D', self.device, str(wav)],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                deadline = time.monotonic() + 60
                while player.poll() is None:
                    # Yield if the user starts a policy during the announcement.
                    if time.monotonic() >= deadline or not robot_idle():
                        player.terminate()
                        return False
                    time.sleep(0.2)
                if player.returncode:
                    raise RuntimeError('audio device busy or unavailable')
            finally:
                if player.poll() is None:
                    player.terminate()
                try:
                    player.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    player.kill(); player.wait()
            self.last_error = None
            LOG.info('Announced Wi-Fi address %s', address)
            return True
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
            error = str(exc)
            if error != self.last_error:
                LOG.warning('Address announcement unavailable: %s', error)
                self.last_error = error
            return False


def write_status(path, address, hostname, announced):
    value = {'wifi_ip':address, 'hostname':hostname, 'announced_ip':announced,
             'control_url': 'http://' + hostname + ':9527/' if hostname else None}
    temporary = path.with_suffix('.new')
    temporary.write_text(json.dumps(value) + '\n')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, default=Path('/run/easyduck-network'))
    parser.add_argument('--voice', default='cmn-latn-pinyin')
    parser.add_argument('--config', type=Path, default=Path('/etc/robot/robotd.toml'))
    parser.add_argument('--render', type=Path, help='Render an address WAV without playback or monitoring')
    parser.add_argument('--address', help='IPv4 address for --render')
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format='%(levelname)s %(message)s')
    if args.render:
        render_speech(args.address, args.render, args.voice)
        return
    with args.config.open('rb') as stream:
        audio = tomllib.load(stream).get('audio', {})
    enabled = audio.get('enabled', True)
    speaker = Speaker(args.runtime, args.voice, audio.get('device', 'plughw:0,0'))
    announcer = Announcer(speaker)
    args.runtime.mkdir(parents=True, exist_ok=True)
    stop = False
    def finish(*_):
        nonlocal stop
        stop = True
    signal.signal(signal.SIGTERM, finish)
    signal.signal(signal.SIGINT, finish)
    while not stop:
        address = addresses()['wifi_host']
        announcer.tick(address, idle=enabled and robot_idle() and pcm_idle())
        write_status(args.runtime / 'status.json', address, mdns_name(), announcer.announced)
        for _ in range(30):
            if stop: break
            time.sleep(0.1)


if __name__ == '__main__':
    main()
