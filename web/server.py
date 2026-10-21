#!/usr/bin/env python3
"""XGO web controls over the existing robotd IPC. Never opens a hardware bus."""
from __future__ import annotations

import argparse
import base64
from collections import deque
import hashlib
import json
import os
import math
from pathlib import Path
import signal
import socket
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit, parse_qs

IDS = [20, 21, 22, 23, 24, 30, 31, 32, 33, 34, 10, 11, 12, 13, 14]
HEAD_KEYS = ('neck_pitch', 'head_pitch', 'head_yaw', 'head_roll')
ASSETS = Path(__file__).resolve().parent / 'assets'


class ApiError(Exception):
    def __init__(self, detail, status=400):
        super().__init__(detail)
        self.status = status


def dumps(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(',', ':'))


class Rpc:
    """Newline JSON-RPC, a dedicated request connection, no action retries."""
    def __init__(self, path):
        self.path, self.seq = path, 0
        self.lock = threading.Lock()
        self.sock = self.reader = None

    def close(self):
        if self.reader is not None:
            self.reader.close()
        if self.sock is not None:
            self.sock.close()
        self.sock = self.reader = None

    def call(self, method, params=None):
        with self.lock:
            try:
                if self.sock is None:
                    self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    self.sock.settimeout(2)
                    self.sock.connect(self.path)
                    self.reader = self.sock.makefile('rb')
                self.seq += 1
                self.sock.sendall((dumps(dict(jsonrpc='2.0', id=self.seq, method=method,
                                             params=params or {})) + '\n').encode())
                line = self.reader.readline(1024 * 1024)
                if not line or not line.endswith(b'\n'):
                    raise OSError('robotd回复中断或过长')
                answer = json.loads(line)
                if answer.get('id') != self.seq:
                    raise OSError('robotd回复ID不匹配')
                if 'error' in answer:
                    raise ApiError(answer['error'].get('message', 'robotd拒绝请求'), 409)
                result = answer.get('result')
                if isinstance(result, dict) and result.get('accepted') is False:
                    raise ApiError(result.get('reason') or 'robotd拒绝请求', 409)
                return result
            except (OSError, ValueError) as exc:
                self.close()
                raise ApiError(f'robotd通信失败：{exc}；没有自动重发动作', 503) from exc


class Snapshot:
    def __init__(self):
        self.lock = threading.Lock()
        self.state, self.models, self.health = {}, {}, {}
        self.received, self.error, self.log = 0.0, '', ''
        self.times = deque(maxlen=100)

    def update(self, state):
        with self.lock:
            self.state = state
            self.received = time.monotonic()
            self.times.append(self.received)
            self.error = ''

    def view(self):
        with self.lock:
            age = time.monotonic() - self.received if self.received else None
            hz = ((len(self.times) - 1) / (self.times[-1] - self.times[0])
                  if len(self.times) > 1 and self.times[-1] > self.times[0] else None)
            return dict(state=self.state, models=self.models, health=self.health,
                        state_age_s=age, received_hz=hz, connection_error=self.error,
                        recent_log=self.log, ids=IDS)


CALIBRATION = '/etc/robot/visual_calibration.json'
ROBOTD = '/opt/robot/daemon/bin/robotd'
# robotd builds known to load the web calibration at startup (same list as the calibration page).
CALIBRATION_READERS = {'6cddf3a2b0bcbbc71dc783fb1b0312849d2f69b5bb223090414cac309827f1d2',
                       '99bcb02b2bea293159b94ea0cd7bee971497057cf2b9bd749d7a9592ca6a6eff'}
CALIBRATION_MARKER = b'cannot read visual calibration'


class CalibrationStatus:
    """Infers whether robotd runs with the saved web calibration; files and systemd only, no IPC."""
    def __init__(self, ttl=2.0):
        self.lock, self.ttl = threading.Lock(), ttl
        self.cached, self.cached_at, self.binaries = None, 0.0, {}

    def binary(self, path):
        st = os.stat(path)
        key = (path, st.st_mtime_ns, st.st_size, st.st_ino)
        if key not in self.binaries:
            data = Path(path).read_bytes()
            sha = hashlib.sha256(data).hexdigest()
            self.binaries = {key: (sha, sha in CALIBRATION_READERS or CALIBRATION_MARKER in data)}
        return self.binaries[key]

    def read(self):
        out = dict(state='unknown', file_sha256=None, saved_at=None, file_mtime=None,
                   robotd_pid=None, robotd_started=None, robotd_sha256=None, robotd_reads=None,
                   inferred=True, error='')
        try:
            data = Path(CALIBRATION).read_bytes()
            out['file_sha256'] = hashlib.sha256(data).hexdigest()[:8]
            out['file_mtime'] = os.stat(CALIBRATION).st_mtime
            out['saved_at'] = json.loads(data).get('saved_at')
        except (OSError, ValueError, AttributeError) as exc:
            out['error'] = f'标定文件不可读：{exc}'
        try:
            r = subprocess.run(['systemctl', 'show', 'robotd', '-p', 'MainPID', '-p', 'ExecMainStartTimestampMonotonic'],
                               capture_output=True, text=True, timeout=2)
            props = dict(line.split('=', 1) for line in r.stdout.splitlines() if '=' in line)
            pid = int(props.get('MainPID') or 0)
            mono = int(props.get('ExecMainStartTimestampMonotonic') or 0)
            if pid > 0:
                out['robotd_pid'] = pid
                # The board's RTC is wrong until NTP syncs, so systemd's wall-clock start time can be
                # months off after a boot. Rebase the monotonic start onto the current wall clock.
                out['robotd_started'] = time.time() - (time.monotonic() - mono / 1e6) if mono else None
                # Hash the running image; fall back to the installed path if /proc is unreadable.
                try:
                    sha, reads = self.binary(f'/proc/{pid}/exe')
                except OSError:
                    sha, reads = self.binary(ROBOTD)
                out['robotd_sha256'], out['robotd_reads'] = sha, reads
            else:
                out['error'] = out['error'] or 'robotd未运行'
        except (OSError, ValueError, subprocess.TimeoutExpired) as exc:
            out['error'] = out['error'] or f'无法读取robotd信息：{exc}'
        if out['robotd_reads'] is False:
            out['state'] = 'unsupported'
        elif out['robotd_reads'] and out['file_mtime'] is not None and out['robotd_started'] is not None:
            out['state'] = 'applied' if out['file_mtime'] < out['robotd_started'] else 'pending_restart'
        return out

    def view(self):
        with self.lock:
            now = time.monotonic()
            if self.cached is None or now - self.cached_at > self.ttl:
                self.cached, self.cached_at = self.read(), now
            return dict(self.cached)


def padd_active():
    # padd 常驻运行不等于手柄已连接；以 robotctl 的真实连接状态为准。
    try:
        result = subprocess.run(
            ['/opt/robot/daemon/current/bin/robotctl', 'pad', 'status'],
            capture_output=True, text=True, timeout=1,
        )
        if result.returncode:
            raise ApiError('无法确认手柄连接状态，请检查robotctl', 503)
        return any(
            line.strip().lower().startswith('pad ') and
            ' connected' in line.strip().lower() and
            'not connected' not in line.strip().lower()
            for line in result.stdout.splitlines()
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ApiError(f'无法确认手柄连接状态：{exc}', 503) from exc


def vector(value, limits):
    if not isinstance(value, list) or len(value) != len(limits):
        raise ApiError('指令维数不正确')
    if any(isinstance(v, bool) or not isinstance(v, (float, int)) or
           not math.isfinite(v) or abs(v) > limit for v, limit in zip(value, limits)):
        raise ApiError('指令不是有效数值或超出原版网页范围')
    return value


class Gateway:
    """Web input adapter; robotd owns arbitration and connection-loss handling."""
    def __init__(self, rpc, snapshot, pad_check=padd_active, clock=time.monotonic):
        self.rpc, self.snapshot = rpc, snapshot
        self.calibration = CalibrationStatus()
        self.pad_check, self.clock = pad_check, clock
        self.lock = threading.RLock()
        self.owner, self.heartbeat = None, 0.0
        self.mode = 'robotd'
        self.command = [0.0] * 3
        self.head = [0.0] * 4
        self.mouth = 0.0
        self.last_error, self.last_request = '', None
        self.session, self.session_at = None, 0.0
        self.control = {}
        self.generation = None
        self.neutral = True
        self.preference = 'auto'
        self.connection_token = None

    def open_connection(self, client, token):
        with self.lock:
            if not self.presence(client, self.neutral):
                return False
            self.connection_token = token
            return True

    def connection_alive(self, client, token):
        """Native WebSocket pong renews presence even when page timers are suspended."""
        with self.lock:
            if self.connection_token is not token or self.session != client:
                return False
            if not self.presence(client, self.neutral):
                return False
            if (self.owner == client and self.control.get('owner') == 'web'
                    and self.control.get('ready') and self.clock()-self.heartbeat > .2):
                # Preserve policy/recovery, but never replay a stale walking velocity.
                self.rpc.call('robot.xgo.command',
                    {'twist':[0.0]*3, 'head':self.head, 'mouth':self.mouth/30})
                self.command = [0.0]*3
            return True

    def close_connection(self, client, token):
        with self.lock:
            if self.connection_token is token:
                self.connection_token = None
                self.request('release', {}, client)

    def local_control(self):
        """Fallback arbitration for robotd builds without robot.control.* IPC."""
        pad = self.pad_check()
        owner = 'web' if self.owner is not None else ('pad' if pad else None)
        return {'owner': owner, 'ready': owner == 'web',
                'generation': self.generation or 0, 'pad_connected': pad,
                'preference': self.preference,
                'reason': 'pad_connected' if pad else 'no_pad',
                'web_connected': self.owner is not None}

    def update_control(self, value):
        if self.generation is not None and value.get('generation') != self.generation:
            self.owner = None
            self.mode = 'robotd'
            self.command, self.head, self.mouth = [0.0]*3, [0.0]*4, 0.0
        self.generation = value.get('generation')
        self.control = value
        return value

    def presence(self, client, neutral, claim=False):
        if not client or len(client)>80:
            raise ApiError('控制页面标识无效，请刷新页面')
        now = self.clock()
        if self.session and self.session != client:
            if not claim and now-self.session_at <= 1:
                return False
            try:
                self.rpc.call('robot.control.heartbeat', {'source':'web','connected':False,'neutral':True})
            except ApiError:
                pass
            self.owner = None
            self.mode = 'robotd'
        self.session, self.session_at, self.neutral = client, now, neutral
        try:
            self.update_control(self.rpc.call('robot.control.heartbeat',
                {'source':'web','connected':True,'neutral':neutral}))
        except ApiError as exc:
            if 'unknown method' not in str(exc):
                raise
            self.control = self.local_control()
        return True

    def request(self, route, body, client):
        if not isinstance(body, dict):
            raise ApiError('需要JSON对象')
        with self.lock:
            if route == 'release':
                if client == self.session:
                    try:
                        self.update_control(self.rpc.call('robot.control.heartbeat',
                            {'source':'web','connected':False,'neutral':True}))
                    except ApiError as exc:
                        if 'unknown method' not in str(exc):
                            raise
                        self.control = self.local_control()
                    self.session, self.owner = None, None
                    self.connection_token = None
                    self.mode = 'robotd'
                return {'ok':True}
            if route == 'control':
                preference = body.get('preference')
                if set(body) != {'preference'} or preference not in ('web','auto'):
                    raise ApiError('未知控制入口')
                if not self.presence(client, self.neutral, claim=preference=='web'):
                    raise ApiError('另一控制页面在线',409)
                self.preference = preference
                try:
                    return self.update_control(self.rpc.call('robot.control.select', {'preference':preference}))
                except ApiError as exc:
                    if 'unknown method' not in str(exc):
                        raise
                    self.owner = client if preference == 'web' else None
                    self.control = self.local_control()
                    return self.control
            if route == 'mode' and body == {'mode':'off'}:
                result = self.rpc.call('robot.relax')
                self.owner, self.mode = None, 'off'
                return {'operation':'off','result':result}
            if route == 'command':
                if set(body) != {'twist','head','mouth'}:
                    raise ApiError('指令字段不正确')
                twist = vector(body['twist'], [.4,.3,1])
                head = vector(body['head'], [1]*4)
                mouth = vector([body['mouth']], [30])[0]
                if mouth < 0:
                    raise ApiError('嘴部角度范围为0～30')
                neutral = all(abs(x)<1e-6 for x in twist+head+[mouth])
                if not self.presence(client,neutral):
                    return {'ok':True,'forwarded':False}
                if self.control.get('owner') != 'web' or not self.control.get('ready') or self.owner != client:
                    return {'ok':True,'forwarded':False}
                self.rpc.call('robot.xgo.command', {'twist':twist,'head':head,'mouth':mouth/30})
                self.command, self.head, self.mouth = twist,head,mouth
                self.heartbeat = self.clock()
                self.last_error = ''
                return {'ok':True,'forwarded':True}
            if route not in ('mode','pick'):
                raise ApiError('接口不存在',404)
            if self.session != client:
                raise ApiError('请先在此页面接管控制',409)
            try:
                self.update_control(self.rpc.call('robot.control.status'))
            except ApiError as exc:
                if 'unknown method' not in str(exc):
                    raise
                self.control = self.local_control()
            if self.control.get('owner') != 'web' or not self.control.get('ready'):
                raise ApiError('当前由手柄控制，或正在等待摇杆归零',409)
            if route == 'mode':
                mode = body.get('mode')
                if set(body) != {'mode'} or mode not in ('hold','policy'):
                    raise ApiError('未知模式')
                self.rpc.call('robot.xgo.command', {'twist':[0.0]*3,'head':[0.0]*4,'mouth':0.0})
                if mode == 'hold':
                    self.rpc.call('robot.enable', {'on':False,'toggle':False})
                    result = self.rpc.call('robot.init')
                else:
                    result = self.rpc.call('robot.enable', {'on':True,'toggle':False})
                self.owner, self.heartbeat, self.mode = client,self.clock(),mode
                self.last_request = {'operation':mode,'result':result}
                self.last_error = ''
                return self.last_request
            if body:
                raise ApiError('捡物不接受额外参数')
            result = self.rpc.call('robot.do', {'skill':'ground_pick'})
            return {'operation':'pick','result':result,'model':self.snapshot.view()['models'].get('ground_pick')}

    def expire(self):
        with self.lock:
            if self.session and self.clock()-self.session_at > 1:
                self.session, self.owner = None,None
                self.connection_token = None
                self.mode = 'robotd'
                try:
                    self.update_control(self.rpc.call('robot.control.heartbeat',
                        {'source':'web','connected':False,'neutral':True}))
                except ApiError as exc:
                    if 'unknown method' not in str(exc):
                        self.last_error = str(exc)

    def status(self, client):
        with self.lock:
            try:
                self.update_control(self.rpc.call('robot.control.status'))
            except ApiError as exc:
                if 'unknown method' not in str(exc):
                    self.last_error = str(exc)
                else:
                    self.control = self.local_control()
            view = self.snapshot.view()
            state, health, models = view['state'], view['health'], view['models']
            gravity = state.get('safety', {}).get('gravity')
            policy = state.get('policy')
            angle = None
            if gravity and math.sqrt(sum(v*v for v in gravity)) > 0:
                angle = math.degrees(math.acos(max(-1, min(1,
                    -gravity[2] / math.sqrt(sum(v*v for v in gravity))))))
            return {**view, 'control': self.control, 'this_page': client==self.session, 'mode': self.mode, 'enabled': None,
                    'feedback_hz': view['received_hz'], 'inference_hz': None,
                    'loop_hz': state.get('loop', {}).get('hz'), 'servo_ids': None,
                    'imu_ok': health.get('imu', {}).get('ready'),
                    'feedback_age_ms': view['state_age_s'] * 1000 if view['state_age_s'] is not None else None,
                    'last_error': view['connection_error'] or self.last_error,
                    'command': self.command, 'head': self.head, 'mouth': self.mouth,
                    'motors': IDS, 'angles': [math.degrees(x) for x in state.get('joints', [])],
                    'targets': [math.degrees(x) for x in state.get('targets', [])],
                    'gyro': state.get('safety', {}).get('base_ang_vel'), 'gravity': gravity,
                    'acc': None, 'recovery': {'active': policy in ('xgo_getup', 'xgo_default_pose'), 'phase': policy, 'angle_deg': angle},
                    'pick': {'active': policy == 'xgo_pick'}, 'roulade': {'active': False},
                    'active_model': models.get('ground_pick') if policy == 'xgo_pick' else
                                    models.get('sitstand') if policy in ('xgo_getup','xgo_default_pose') else
                                    models.get('walk') if policy == 'xgo_walk' else
                                    models.get('stand') if policy == 'stand' else None,
                    'last_request': self.last_request, 'web_active': self.owner is not None,
                    'backend': 'xgoduck control flow in robotd; TaishanPi 1910 hardware', 'torque': None,
                    'calibration': self.calibration.view()}


def background(gateway, path, stop):
    snap = gateway.snapshot
    health_rpc = Rpc(path)

    def subscribe():
        while not stop.is_set():
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                    sock.settimeout(3)
                    sock.connect(path)
                    sock.sendall((dumps(dict(jsonrpc='2.0', id=1, method='robot.subscribe',
                                             params={'hz': 10})) + '\n').encode())
                    with sock.makefile('rb') as reader:
                        while not stop.is_set():
                            line = reader.readline(1024 * 1024)
                            if not line or not line.endswith(b'\n'):
                                raise OSError('robotd状态流中断')
                            msg = json.loads(line)
                            if msg.get('method') == 'robot.state':
                                snap.update(msg['params'])
                            elif msg.get('id') == 1:
                                if 'error' in msg:
                                    raise OSError(str(msg['error']))
                                with snap.lock:
                                    snap.models = msg.get('result', {})
            except (OSError, ValueError, KeyError) as exc:
                with snap.lock:
                    snap.error = str(exc)
                stop.wait(1)

    threading.Thread(target=subscribe, name='robot-state', daemon=True).start()
    next_poll = 0.0
    while not stop.wait(.1):
        gateway.expire()
        if time.monotonic() >= next_poll:
            next_poll = time.monotonic() + 2
            try:
                health = health_rpc.call('robot.health')
                with snap.lock:
                    snap.health = health
                log = subprocess.run(['journalctl', '-u', 'robotd', '-n', '20', '--no-pager', '-o', 'short-iso'],
                                     capture_output=True, text=True, timeout=2)
                with snap.lock:
                    snap.log = log.stdout[-10000:] if log.returncode == 0 else log.stderr[-1000:]
            except (ApiError, OSError, subprocess.TimeoutExpired) as exc:
                with snap.lock:
                    snap.error = str(exc)


def make_handler(gateway):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):
            # Frequent status requests are not user actions; omit body/headers from logs.
            if self.command == 'POST':
                print(f'{self.address_string()} {self.path} {args[1] if len(args)>1 else ""}', flush=True)

        def reply(self, value, status=200, mime='application/json; charset=utf-8'):
            data = value if isinstance(value, bytes) else dumps(value).encode()
            self.send_response(status)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Cache-Control', 'no-store')
            self.send_header('X-Content-Type-Options', 'nosniff')
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            route = urlsplit(self.path).path
            if route == '/api/connection':
                try:
                    return self.control_connection()
                except (ValueError, ApiError) as exc:
                    return self.reply({'detail':str(exc)}, status=getattr(exc,'status',400))
            if route == '/api/status':
                return self.reply(gateway.status(self.headers.get('X-Control-Id', '')))
            file = {'/': ('index.html', 'text/html; charset=utf-8'),
                    '/app.js': ('app.js', 'text/javascript; charset=utf-8'),
                    '/style.css': ('style.css', 'text/css; charset=utf-8')}.get(route)
            if file:
                return self.reply((ASSETS / file[0]).read_bytes(), mime=file[1])
            self.reply({'detail': '不存在'}, 404)

        def control_connection(self):
            """A native ping/pong lease; no JavaScript timer or motion messages required."""
            origin=urlsplit(self.headers.get('Origin',''))
            if origin.scheme not in ('http','https') or origin.netloc != self.headers.get('Host'):
                raise ApiError('连接来源不匹配',403)
            if (self.headers.get('Upgrade','').lower() != 'websocket'
                    or 'upgrade' not in [v.strip() for v in self.headers.get('Connection','').lower().split(',')]
                    or self.headers.get('Sec-WebSocket-Version') != '13'):
                raise ApiError('需要WebSocket连接')
            key=self.headers.get('Sec-WebSocket-Key','')
            if len(base64.b64decode(key,validate=True)) != 16:
                raise ApiError('WebSocket连接标识无效')
            client=parse_qs(urlsplit(self.path).query).get('client',[''])[0]
            token=object()
            if not gateway.open_connection(client,token):
                raise ApiError('另一控制页面在线',409)
            self.protocol_version='HTTP/1.1'
            self.close_connection=True
            try:
                self.send_response(101)
                self.send_header('Upgrade','websocket')
                self.send_header('Connection','Upgrade')
                accept=base64.b64encode(hashlib.sha1(
                    (key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
                self.send_header('Sec-WebSocket-Accept',accept)
                self.end_headers()
                self.connection.settimeout(.8)
                while True:
                    challenge=os.urandom(8)
                    self.connection.sendall(bytes([0x89,len(challenge)])+challenge)
                    while True:
                        header=self.rfile.read(2)
                        if len(header)!=2:
                            return
                        first,second=header
                        size=second&127
                        if first&0xf0 != 0x80 or not second&128 or size>125:
                            return
                        mask=self.rfile.read(4);data=self.rfile.read(size)
                        if len(mask)!=4 or len(data)!=size:
                            return
                        data=bytes(v^mask[i%4] for i,v in enumerate(data))
                        opcode=first&15
                        if opcode==8:
                            self.connection.sendall(bytes([0x88,len(data)])+data)
                            return
                        if opcode==9:
                            self.connection.sendall(bytes([0x8a,len(data)])+data)
                            continue
                        if opcode!=10 or data!=challenge:
                            return
                        break
                    if not gateway.connection_alive(client,token):
                        return
                    time.sleep(.2)
            except (OSError, ApiError):
                pass
            finally:
                gateway.close_connection(client,token)

        def do_POST(self):
            try:
                origin = self.headers.get('Origin')
                if origin and (urlsplit(origin).netloc != self.headers.get('Host') or
                               urlsplit(origin).scheme not in ('http', 'https')):
                    raise ApiError('只接受当前网页来源的控制请求', 403)
                if self.headers.get_content_type() != 'application/json':
                    raise ApiError('需要application/json', 415)
                size = int(self.headers.get('Content-Length', '0'))
                if not 0 < size <= 4096:
                    raise ApiError('请求体长度不正确', 413)
                client = self.headers.get('X-Control-Id','')
                path = urlsplit(self.path).path
                if path not in ('/api/mode', '/api/command', '/api/pick', '/api/control', '/api/release'):
                    raise ApiError('接口不存在', 404)
                body = json.loads(self.rfile.read(size))
                result = gateway.request(path[5:], body, client)
                self.reply(result)
            except (ValueError, ApiError) as exc:
                with gateway.lock:
                    gateway.last_error = str(exc)
                self.reply({'detail': str(exc)}, getattr(exc, 'status', 400))
            except Exception as exc:
                with gateway.lock:
                    gateway.last_error = str(exc)
                self.reply({'detail': '网页接口异常，请查看服务日志'}, 500)
                print(f'HTTP error: {exc}', flush=True)

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host', default='0.0.0.0')
    parser.add_argument('--port', type=int, default=9527)
    parser.add_argument('--socket', default='/run/robotd.sock')
    args = parser.parse_args()
    stop = threading.Event()
    gateway = Gateway(Rpc(args.socket), Snapshot())
    server = ThreadingHTTPServer((args.host, args.port), make_handler(gateway))
    server.daemon_threads = True
    threading.Thread(target=background, args=(gateway, args.socket, stop), daemon=True).start()

    def shutdown(*_):
        stop.set()
        threading.Thread(target=server.shutdown, daemon=True).start()

    signal.signal(signal.SIGTERM, shutdown)
    signal.signal(signal.SIGINT, shutdown)
    print(f'XGO web -> existing robotd, http://{args.host}:{args.port}; startup sends no motion', flush=True)
    try:
        server.serve_forever(poll_interval=.2)
    finally:
        stop.set()
        with gateway.lock:
            if gateway.session is not None:
                try:
                    gateway.request('release', {}, gateway.session)
                except ApiError:
                    pass  # robotd independently expires the web heartbeat.
        server.server_close()
        gateway.rpc.close()


if __name__ == '__main__':
    main()
