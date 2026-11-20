#!/usr/bin/env python3
"""Display EasyDuck status on the first WS2812B pixel on SPI3 MOSI."""
import fcntl
import json
import logging
import math
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import threading
import time

DEVICE = '/dev/spidev3.0'
ROBOT_SOCKET = '/run/robotd.sock'
SETTINGS = Path('/var/lib/easyduck/battery-led/settings.json')
STATUS = Path('/run/easyduck-battery-led/status.json')
CALIBRATION = Path('/etc/robot/visual_calibration.json')
CALIBRATION_DIR = Path('/opt/robot/calibration')
if not CALIBRATION_DIR.is_dir():
    CALIBRATION_DIR = Path(__file__).resolve().parents[2] / 'calibration'
sys.path.insert(0, str(CALIBRATION_DIR))
from calibration_math import HOME, validate_profile
from imu import calibration as imu_calibration, configuration as imu_configuration

SPI_SPEED = 3_200_000
SPI_WRITE_MODE = 0x40016B01
SPI_WRITE_BITS = 0x40016B03
SPI_WRITE_SPEED = 0x40046B04
# At 3.2 MHz each SPI bit takes about 0.31 us. Reset is low for 320 us.
RESET = bytes(128)
OFF = (0, 0, 0)
CYCLE_SECONDS = (5.0, 4.0, 3.0, 2.0, 1.5)
DEFAULT_SPEED = 5
MIN_INTENSITY = 0.10
FRAME_INTERVAL = 0.05
SAMPLE_INTERVAL = 0.5
FAULT_DELAY = 3.0
PAD_DISCONNECT_DELAY = 1.0


def pixel_frame(rgb):
    red, green, blue = rgb
    encoded = bytearray()
    for value in (green, red, blue):
        for shift in (4, 0):
            nibble = value >> shift
            encoded.append(
                (0xC0 if nibble & 8 else 0x80) |
                (0x0C if nibble & 4 else 0x08)
            )
            encoded.append(
                (0xC0 if nibble & 2 else 0x80) |
                (0x0C if nibble & 1 else 0x08)
            )
    return RESET + encoded + RESET


def show(fd, color):
    frame = pixel_frame(color)
    if os.write(fd, frame) != len(frame):
        raise OSError('Incomplete WS2812B transfer')


def calibration_ready():
    try:
        profile = validate_profile(json.loads(CALIBRATION.read_text()))
        if not profile.get('saved_at'):
            return False
        reference = profile.get('reference_pose', {})
        manifest = json.loads((CALIBRATION_DIR / 'static/model/manifest.json').read_text())
        model = imu_configuration('/etc/robot/robotd.toml')['model']
        return (reference.get('model_sha256') == manifest['source_sha256']
                and reference.get('angles') == HOME
                and imu_calibration(profile, model)['mount_valid'])
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return False


def robot_snapshot():
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
            sock.settimeout(0.4)
            sock.connect(ROBOT_SOCKET)
            with sock.makefile('rb') as reader:
                def receive():
                    line = reader.readline(65536)
                    if not line.endswith(b'\n'):
                        raise OSError('Incomplete robotd response')
                    return json.loads(line)

                def call(method, request_id, params):
                    msg = dict(jsonrpc='2.0', id=request_id, method=method, params=params)
                    sock.sendall(json.dumps(msg).encode() + b'\n')
                    answer = receive()
                    if answer.get('id') != request_id:
                        raise ValueError('Unexpected robotd response')
                    return answer['result']

                health = call('robot.health', 1, {})
                control = call('robot.control.status', 2, {})
                call('robot.subscribe', 3, {'hz': 10})
                event = receive()
                if event.get('method') != 'robot.state':
                    raise ValueError('Missing robot state')
                return health, control, event['params']
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return None


def read_settings():
    try:
        settings = json.loads(SETTINGS.read_text())
    except (OSError, ValueError):
        settings = {}
    if not isinstance(settings, dict):
        settings = {}
    brightness = settings.get('brightness', 70)
    if type(brightness) is not int or not 1 <= brightness <= 100:
        brightness = 70
    speed = settings.get('speed', DEFAULT_SPEED)
    if type(speed) is not int or not 1 <= speed <= len(CYCLE_SECONDS):
        speed = DEFAULT_SPEED
    return settings.get('enabled') is not False, brightness, CYCLE_SECONDS[speed - 1]


def battery_percent(health):
    try:
        if not 0 <= health['control_loop']['last_tick_age_ms'] < 2000:
            return None
        if health['bus']['consecutive_errors'] >= 5:
            return None
        percent = float(health['battery']['percent'])
        voltage = float(health['battery']['volts'])
        if math.isfinite(voltage) and voltage > 0 and 0 <= percent <= 100:
            return percent
    except (KeyError, TypeError, ValueError):
        pass
    return None


BATTERY_GREEN = 'battery_green'
BATTERY_YELLOW = 'battery_yellow'
BATTERY_RED = 'battery_red'
PATTERNS = {
    'low_battery_protection': ((32, 0, 0), 'fast', 0.4),
    'fault': ((24, 0, 28), 'double', 1.7),
    'pad_connected': ((18, 18, 18), 'double', 1.2),
    'pad_disconnected': ((18, 18, 18), 'solid', 1.0),
    'calibration_pending': ((0, 0, 28), 'breath', 5.0),
    BATTERY_GREEN: ((0, 28, 0), 'breath', None),
    BATTERY_YELLOW: ((32, 24, 0), 'breath', None),
    BATTERY_RED: ((32, 0, 0), 'breath', None),
}


class Indicator:
    def __init__(self):
        self.fault_since = None
        self.pad = None
        self.disconnect_since = None
        self.notice = None
        self.notice_until = 0.0

    def state(self, now, snapshot, calibrated, enabled, calibration_active=False):
        health, control, robot = snapshot or ({}, {}, {})
        low = robot.get('policy') in ('low_battery_sitting', 'low_battery_locked')
        fault = ((snapshot is None and calibrated and not calibration_active)
                 or (bool(health) and health.get('healthy') is False))
        if fault and self.fault_since is None:
            self.fault_since = now
        elif not fault:
            self.fault_since = None
        fault_active = self.fault_since is not None and now - self.fault_since >= FAULT_DELAY

        connected = control.get('pad_connected')
        if type(connected) is bool:
            if connected:
                self.disconnect_since = None
                if self.pad is not True:
                    self.notice, self.notice_until = 'pad_connected', now + 1.2
                self.pad = True
            elif self.pad is True:
                if self.disconnect_since is None:
                    self.disconnect_since = now
                elif now - self.disconnect_since >= PAD_DISCONNECT_DELAY:
                    self.pad = False
                    self.notice, self.notice_until = 'pad_disconnected', now + 1.0
            else:
                self.pad = False
        else:
            self.disconnect_since = None

        if low or fault_active or not enabled:
            self.notice = None

        if not enabled:
            return 'disabled'
        if low:
            return 'low_battery_protection'
        if fault_active:
            return 'fault'
        if self.notice and now < self.notice_until:
            return self.notice
        if not calibrated:
            return 'calibration_pending'
        percent = battery_percent(health)
        if percent is None:
            return 'battery_unknown'
        if percent >= 50:
            return BATTERY_GREEN
        if percent >= 20:
            return BATTERY_YELLOW
        return BATTERY_RED


def render_state(state, elapsed, brightness, battery_cycle):
    pattern = PATTERNS.get(state)
    if pattern is None:
        return OFF
    color, mode, cycle = pattern
    peak = tuple(max(1, round(channel * brightness / 100)) if channel else 0
                 for channel in color)
    if mode == 'breath':
        return breathing_color(peak, elapsed, cycle or battery_cycle)
    if mode == 'fast':
        return peak if elapsed % cycle < 0.2 else OFF
    if mode == 'double':
        phase = elapsed % cycle
        return peak if phase < 0.15 or 0.3 <= phase < 0.45 else OFF
    return peak


def publish_state(state):
    temporary = STATUS.with_suffix('.new')
    temporary.write_text(json.dumps({'state': state}) + '\n')
    temporary.replace(STATUS)


def sample_robot(stop, samples):
    while not stop.is_set():
        snapshot = robot_snapshot()
        calibrated = calibration_ready()
        calibrating = False
        if snapshot is None and calibrated:
            try:
                calibrating = subprocess.run(
                    ['systemctl', 'is-active', '--quiet', 'xgoduck-calibration.service'],
                    timeout=1, check=False,
                ).returncode == 0
            except (OSError, subprocess.TimeoutExpired):
                pass
        samples[0] = (snapshot, calibrated, calibrating, read_settings())
        stop.wait(SAMPLE_INTERVAL)


def breathing_color(peak, elapsed, cycle):
    if peak == OFF:
        return OFF
    phase = (elapsed % cycle) / cycle
    wave = (1 - math.cos(math.tau * phase)) / 2
    intensity = MIN_INTENSITY + (1 - MIN_INTENSITY) * wave
    red, green, blue = (
        max(1, round(channel * intensity)) if channel else 0
        for channel in peak
    )
    return red, green, blue


def main():
    logging.basicConfig(level=logging.INFO, format='%(message)s')
    stop = threading.Event()
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda *_: stop.set())
    fd = os.open(DEVICE, os.O_WRONLY)
    try:
        fcntl.ioctl(fd, SPI_WRITE_MODE, b'\0')
        fcntl.ioctl(fd, SPI_WRITE_BITS, b'\x08')
        fcntl.ioctl(fd, SPI_WRITE_SPEED, struct.pack('=I', SPI_SPEED))
        previous_frame = None
        previous_state = None
        previous_cycle = None
        indicator = Indicator()
        samples = [(None, calibration_ready(), False, read_settings())]
        threading.Thread(target=sample_robot, args=(stop, samples), daemon=True).start()
        started = time.monotonic()
        try:
            while not stop.is_set():
                now = time.monotonic()
                snapshot, calibrated, calibrating, settings = samples[0]
                enabled, brightness, cycle = settings
                state = indicator.state(now, snapshot, calibrated, enabled, calibrating)
                if state != previous_state:
                    started = now
                    publish_state(state)
                    logging.info('Indicator state: %s', state)
                    previous_state = state
                elif state in (BATTERY_GREEN, BATTERY_YELLOW, BATTERY_RED) and cycle != previous_cycle:
                    started = now
                previous_cycle = cycle
                frame = render_state(state, now - started, brightness, cycle)
                if frame != previous_frame:
                    show(fd, frame)
                    previous_frame = frame
                stop.wait(FRAME_INTERVAL)
        finally:
            show(fd, OFF)
    finally:
        os.close(fd)


if __name__ == '__main__':
    main()
