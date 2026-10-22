"""Exclusive service handoff between robot control and calibration."""
from contextlib import contextmanager
import json
from pathlib import Path
import socket
import subprocess
import time
import urllib.error
import urllib.request
from connections import SetupError


class Navigator:
    def __init__(self, state_dir='/var/lib/easyduck/setup', control_port=9527, calibration_port=8766):
        self.state_dir = Path(state_dir)
        self.control_port, self.calibration_port = control_port, calibration_port

    def unit(self, verb, *names):
        try:
            p = subprocess.run(['systemctl', verb, *names], capture_output=True, text=True, timeout=25)
        except (OSError, subprocess.TimeoutExpired):
            raise SetupError('service_timeout', '服务切换超时，请查看服务状态后重试。')
        if p.returncode:
            raise SetupError('service_failed', '服务切换失败，请检查：' + ', '.join(names))

    def calibration_active(self):
        p = subprocess.run(['systemctl', 'is-active', 'xgoduck-calibration.service'],
                           capture_output=True, text=True, timeout=5)
        return p.stdout.strip() in ('active', 'activating', 'deactivating')

    def api(self, port, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(f'http://127.0.0.1:{port}{path}', data=data,
                                         headers={'Content-Type': 'application/json'})
        try:
            with urllib.request.urlopen(request, timeout=5) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            try: message = json.load(error).get('error')
            except (ValueError, OSError): message = None
            raise SetupError('handoff_rejected', message or '页面服务拒绝切换，请刷新状态后重试。')
        except (OSError, ValueError):
            raise SetupError('page_unavailable', '目标页面服务尚未就绪，请稍后重试。')

    def wait_ready(self, port, path):
        end = time.monotonic() + 15
        while True:
            try:
                result = self.api(port, path)
                if path == '/api/status' and not result.get('state'):
                    raise SetupError('control_starting', '正式控制正在初始化，请稍候。')
                return result
            except SetupError:
                if time.monotonic() >= end: raise
                time.sleep(.25)

    @staticmethod
    def check_exit(state):
        if state.get('dirty'):
            raise SetupError('unsaved_calibration', '有未保存标定草稿，请先保存或撤销，再切换页面。')
        if state.get('motion', {}).get('active'):
            raise SetupError('calibration_moving', '实物运动任务尚未结束，请先停止运动。')
        rows = state.get('rows', [])
        if len(rows) != 15 or any(row.get('torque') is not False or row.get('error') for row in rows):
            raise SetupError('calibration_powered', '请先在标定页全部松力矩，并确认全部舵机反馈正常，再切换。')

    def relax(self):
        # Use the bus owner's IPC; never open the servo serial port here.
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(5); sock.connect('/run/robotd.sock')
                sock.sendall((json.dumps({'jsonrpc':'2.0','id':1,'method':'robot.relax','params':{}})+'\n').encode())
                with sock.makefile('rb') as stream:
                    result = json.loads(stream.readline(65536))
                if result.get('error') or not result.get('result', {}).get('accepted'):
                    raise ValueError()
        except (OSError, ValueError):
            raise SetupError('relax_failed', '正式控制未确认断力请求，未进入标定。')

    @contextmanager
    def lock(self):
        import fcntl
        self.state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        with (self.state_dir / 'navigation.lock').open('a') as stream:
            try: fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError: raise SetupError('busy', '其他页面正在切换，请稍后重试。')
            try: yield
            finally: fcntl.flock(stream, fcntl.LOCK_UN)

    def switch(self, target, confirm=False):
        if target not in ('setup', 'control', 'calibration'):
            raise SetupError('invalid_target', '未知页面。')
        with self.lock():
            active = self.calibration_active()
            if target == 'calibration':
                if not active:
                    if confirm is not True:
                        raise SetupError('confirm_required', '进入标定会断力并停止正式控制，请先放稳机器人后确认。')
                    self.unit('stop', 'padd.service')
                    try:
                        self.relax()
                    except SetupError:
                        self.unit('start', 'padd.service')
                        raise
                    self.unit('stop', 'xgoduck-web.service', 'robotd.service')
                    try:
                        self.unit('start', 'xgoduck-calibration.service')
                        self.wait_ready(self.calibration_port, '/api/state')
                        self.api(self.calibration_port, '/api/command', {'op':'off_all'})
                    except SetupError:
                        self.unit('stop', 'xgoduck-calibration.service')
                        self.unit('start', 'robotd.service', 'xgoduck-web.service', 'padd.service')
                        raise
                self.wait_ready(self.calibration_port, '/api/state')
                return {'port':self.calibration_port, 'path':'/', 'message':'标定页已就绪。'}
            if active:
                state = self.api(self.calibration_port, '/api/state')
                self.check_exit(state)
                self.api(self.calibration_port, '/api/command',
                         {'op':'prepare_exit', 'revision':state['revision'], 'confirm':True})
                try:
                    self.unit('stop', 'xgoduck-calibration.service')
                except SetupError:
                    try: self.api(self.calibration_port, '/api/command', {'op':'cancel_exit'})
                    except SetupError: pass
                    raise
                check = subprocess.run(['systemctl', 'show', 'xgoduck-calibration.service', '-p', 'Result', '--value'], capture_output=True, text=True, timeout=5)
                if check.returncode or check.stdout.strip() != 'success':
                    raise SetupError('calibration_exit_failed', '标定服务退出异常，未恢复正式控制，请检查速度恢复和串口状态。')
                self.unit('start', 'robotd.service', 'xgoduck-web.service', 'padd.service')
            elif target == 'control':
                self.unit('start', 'robotd.service', 'xgoduck-web.service', 'padd.service')
            if active or target == 'control':
                self.wait_ready(self.control_port, '/api/status')
            return {'port':self.control_port if target == 'control' else None,
                    'path':'/', 'message':'页面已就绪。'}
