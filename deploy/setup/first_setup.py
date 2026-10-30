#!/usr/bin/env python3
"""Authenticated local Wi-Fi and DualSense setup; Python standard library only."""
import argparse
import ctypes
import ctypes.util
import hmac
from http.cookies import SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import html
import json
import os
from pathlib import Path
import sys as _network_sys
_network_root = Path(__file__).resolve().parents[1]
if not (_network_root / 'deploy/network_routes.py').is_file(): _network_root = _network_root.parent
_network_sys.path.insert(0, str(_network_root))
from deploy.network_routes import addresses as network_addresses, client_script as navigation_client
import secrets
import signal
import threading
import time
from urllib.parse import urlsplit, parse_qs
from connections import Backend, SetupError
from navigation import Navigator

STATIC = Path(__file__).with_name('static')

def default_user():
    try:
        import pwd
        return pwd.getpwuid(1000).pw_name
    except (ImportError, KeyError):
        return 'root'

class Auth:
    def __init__(self, username):
        self.username = username
        self.lock = threading.Lock()

    def verify(self, username, password):
        if username != self.username or not isinstance(password, str) or not 1 <= len(password) <= 1024:
            return False
        try:
            import pwd
            if pwd.getpwnam(username).pw_shell.endswith(('/nologin', '/false')):
                return False
            with Path('/etc/shadow').open() as stream:
                fields = next((line.rstrip().split(':') for line in stream if line.startswith(username + ':')), [])
            if len(fields) < 8 or not fields[1] or fields[1].startswith(('!', '*')):
                return False
            today = int(time.time() / 86400)
            if fields[7] and today >= int(fields[7]):
                return False
            if fields[2] == '0' or (fields[2] and fields[4] and int(fields[4]) >= 0 and today > int(fields[2]) + int(fields[4])):
                return False
            library = ctypes.CDLL(ctypes.util.find_library('crypt') or 'libcrypt.so.1')
            library.crypt.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
            library.crypt.restype = ctypes.c_char_p
            with self.lock:
                result = library.crypt(password.encode(), fields[1].encode())
                return bool(result) and hmac.compare_digest(result, fields[1].encode())
        except (OSError, ValueError, ImportError, KeyError):
            return False

class App:
    def __init__(self, backend=None, auth=None, control_port=9527, system_port=None, navigation=None):
        self.backend = backend or Backend()
        self.auth = auth or Auth(os.environ.get('EASYDUCK_SETUP_USER', default_user()))
        self.control_port, self.system_port = control_port, system_port
        self.navigation = navigation or Navigator(getattr(self.backend, "state_dir", "/var/lib/easyduck/setup"), control_port, int(os.environ.get("EASYDUCK_CALIBRATION_PORT", "8766")))
        self.sessions, self.failures, self.jobs = {}, {}, {}
        self.lock, self.cache_lock = threading.Lock(), threading.Lock()
        self.active = False
        self.cache = (0, None)

    def status(self, refresh=False):
        with self.cache_lock:
            at, data = self.cache
            if refresh or data is None or time.monotonic() - at > 3:
                data = self.backend.status()
                self.cache = (time.monotonic(), data)
            return data

    def session(self, cookie):
        try:
            value = SimpleCookie(cookie).get('easyduck_setup')
            token = value.value if value else ''
        except Exception:
            return None
        with self.lock:
            session = self.sessions.get(token)
            if session and session['until'] > time.time():
                return session
            self.sessions.pop(token, None)
        return None

    def login(self, ip, body):
        with self.lock:
            hits = [t for t in self.failures.get(ip, []) if time.time() - t < 300]
            if len(hits) >= 5:
                raise SetupError('rate_limit', '登录尝试过多，请5分钟后重试。')
        if not self.auth.verify(body.get('username', ''), body.get('password', '')):
            with self.lock:
                if len(self.failures) > 512: self.failures.clear()
                self.failures[ip] = hits + [time.time()]
            raise SetupError('login_failed', '账号或密码错误，或该系统账号不可登录。')
        token = secrets.token_urlsafe(32)
        with self.lock:
            self.failures.pop(ip, None)
            self.sessions = {k:v for k,v in self.sessions.items() if v['until'] > time.time()}
            if len(self.sessions) >= 64: self.sessions.pop(next(iter(self.sessions)))
            self.sessions[token] = {'csrf':secrets.token_urlsafe(32), 'until':time.time()+28800, 'token':token}
        return token

    def start_job(self, action, body):
        with self.lock:
            if self.active: raise SetupError('busy', '已有连接操作正在执行，请等待完成。')
            self.active = True
            if len(self.jobs) > 32: self.jobs.clear()
            job_id = secrets.token_hex(12)
            self.jobs[job_id] = {'done':False}
        def work():
            try:
                if action == 'navigation':
                    result = self.navigation.switch(**body)
                else:
                    with self.backend.operation(action.split('/')[0]):
                        if action == 'wifi/scan': result = self.backend.scan_wifi()
                        elif action == 'wifi/connect': result = self.backend.connect_wifi(**body)
                        elif action == 'bluetooth/scan': result = self.backend.scan_bluetooth()
                        else: result = self.backend.gamepad(action.split('/')[1], **body)
                result['ok'] = True
            except SetupError as error:
                result = {'ok':False, 'code':error.code, 'message':str(error)}
            except Exception:
                # Never log command arguments, request bodies or submitted secrets.
                result = {'ok':False, 'code':'service_error', 'message':'连接操作未完成，请检查系统服务后重试。'}
            finally:
                with self.cache_lock:
                    self.cache = (0, None)
                with self.lock:
                    self.jobs[job_id] = {'done':True, 'result':result}
                    self.active = False
        threading.Thread(target=work, daemon=True).start()
        return job_id

def make_handler(app):
    class Handler(BaseHTTPRequestHandler):
        server_version = 'EasyduckSetup/1'
        def log_message(self, *_): pass
        def send(self, code, data, content_type='application/json; charset=utf-8', headers=()):
            if isinstance(data, (dict,list)): data = json.dumps(data, ensure_ascii=False).encode()
            elif isinstance(data, str): data = data.encode()
            self.send_response(code)
            for name,value in [('Content-Type',content_type), ('Content-Length',str(len(data))),
                    ('Cache-Control','no-store'), ('X-Content-Type-Options','nosniff'), ('X-Frame-Options','DENY'),
                    ('Content-Security-Policy',"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; form-action 'self'")]+list(headers):
                self.send_header(name,value)
            self.end_headers(); self.wfile.write(data)
        def redirect(self, path): self.send(302,b'',headers=[('Location',path)])
        def do_GET(self):
            path = urlsplit(self.path).path
            session = app.session(self.headers.get('Cookie',''))
            target = parse_qs(urlsplit(self.path).query).get('target', [''])[0]
            target = target if target in ('setup', 'control', 'calibration', 'servo_setup') else ''
            source = parse_qs(urlsplit(self.path).query).get('source', ['setup'])[0]
            source = source if source in ('setup', 'control', 'calibration') else 'setup'
            if path == '/api/network-addresses': return self.send(200,network_addresses())
            if path == '/navigation-client.js': return self.send(200,navigation_client(),'text/javascript; charset=utf-8')
            if path == '/navigate':
                if not target: return self.send(400, {'ok':False,'message':'未知页面。'})
                if not session: return self.redirect('/login?target='+target+'&source='+source)
                text=(STATIC/'navigation.html').read_text(encoding='utf-8').replace('__CSRF__',html.escape(session['csrf'],quote=True)).replace('__TARGET__',target)
                source_port={'setup':self.server.server_address[1],'control':app.control_port,'calibration':app.navigation.calibration_port}[source]
                text=text.replace('__SOURCE_PORT__',str(source_port))
                return self.send(200,text,'text/html; charset=utf-8')
            if path in ('/','/login'):
                if path=='/' and not session: return self.redirect('/login')
                if path=='/login' and session: return self.redirect('/navigate?target='+target+'&source='+source if target else '/')
                text=(STATIC/('index.html' if path=='/' else 'login.html')).read_text(encoding='utf-8')
                text=text.replace('__CSRF__',html.escape(session['csrf'] if session else '',quote=True)).replace('__USER__',html.escape(app.auth.username,quote=True))
                return self.send(200,text,'text/html; charset=utf-8')
            if path in ('/static/setup.css','/static/setup.js','/static/navigation.js'):
                return self.send(200,(STATIC/path.rsplit('/',1)[1]).read_bytes(), 'text/css; charset=utf-8' if path.endswith('.css') else 'text/javascript; charset=utf-8')
            if not session: return self.send(401,{'ok':False,'message':'登录已过期，请重新登录。'})
            if path=='/api/status':
                try:
                    data=dict(app.status()); address=self.connection.getsockname()[0]
                    data.update(control_url=f'http://{address}:{app.control_port}/',system_url=f'http://{address}:{app.system_port}/' if app.system_port else None,login_user=app.auth.username)
                    return self.send(200,{'ok':True,'data':data})
                except Exception: return self.send(503,{'ok':False,'message':'读取连接状态失败，请检查NetworkManager和BlueZ服务。'})
            if path.startswith('/api/jobs/'):
                with app.lock: job=app.jobs.get(path.rsplit('/',1)[1])
                return self.send(200 if job else 404,job or {'ok':False,'message':'操作记录已过期，请刷新状态。'})
            self.send(404,{'ok':False,'message':'接口不存在。'})
        def do_POST(self):
            path=urlsplit(self.path).path
            try:
                origin=self.headers.get('Origin')
                if origin and urlsplit(origin).netloc!=self.headers.get('Host'):
                    return self.send(403,{'ok':False,'message':'请求来源不一致，请从当前页面操作。'})
                size=int(self.headers.get('Content-Length','0'))
                if not 0<size<=4096 or self.headers.get_content_type()!='application/json': raise ValueError()
                body=json.loads(self.rfile.read(size))
                if not isinstance(body,dict): raise ValueError()
            except (ValueError,UnicodeError): return self.send(400,{'ok':False,'message':'请求格式无效。'})
            if path=='/api/login':
                try:
                    token=app.login(self.client_address[0],body)
                    return self.send(200,{'ok':True},headers=[('Set-Cookie',f'easyduck_setup={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=28800')])
                except SetupError as error: return self.send(429 if error.code=='rate_limit' else 401,{'ok':False,'message':str(error)})
            session=app.session(self.headers.get('Cookie',''))
            if not session: return self.send(401,{'ok':False,'message':'登录已过期，请重新登录。'})
            if not hmac.compare_digest(self.headers.get('X-CSRF-Token',''),session['csrf']):
                return self.send(403,{'ok':False,'message':'请求校验失败，请重新打开页面。'})
            if path=='/api/logout':
                with app.lock: app.sessions.pop(session['token'],None)
                return self.send(200,{'ok':True},headers=[('Set-Cookie','easyduck_setup=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0')])
            if path not in ('/api/wifi/scan','/api/wifi/connect','/api/bluetooth/scan','/api/bluetooth/pair','/api/bluetooth/connect','/api/bluetooth/disconnect','/api/bluetooth/forget','/api/navigation'):
                return self.send(404,{'ok':False,'message':'接口不存在。'})
            try: self.send(202,{'ok':True,'job':app.start_job(path.removeprefix('/api/'),body)})
            except SetupError as error: self.send(409,{'ok':False,'message':str(error)})
    return Handler

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host',default=os.environ.get('EASYDUCK_SETUP_HOST','0.0.0.0'))
    parser.add_argument('--port',type=int,default=int(os.environ.get('EASYDUCK_SETUP_PORT','80')))
    parser.add_argument('--reconnect-once',action='store_true')
    args=parser.parse_args()
    backend=Backend(os.environ.get('EASYDUCK_SETUP_STATE','/var/lib/easyduck/setup'))
    if args.reconnect_once:
        try:
            with backend.operation('bluetooth'): backend.reconnect_once()
        except SetupError: pass
        return
    app=App(backend=backend,control_port=int(os.environ.get('EASYDUCK_CONTROL_PORT','9527')),system_port=int(os.environ['EASYDUCK_SYSTEM_PORT']) if os.environ.get('EASYDUCK_SYSTEM_PORT') else None)
    server=ThreadingHTTPServer((args.host,args.port),make_handler(app));server.daemon_threads=True
    for sig in (signal.SIGTERM,signal.SIGINT): signal.signal(sig,lambda *_:threading.Thread(target=server.shutdown,daemon=True).start())
    print(f'Easyduck connection setup listening on {args.host}:{args.port}',flush=True)
    try: server.serve_forever()
    finally: server.server_close()

if __name__=='__main__': main()
