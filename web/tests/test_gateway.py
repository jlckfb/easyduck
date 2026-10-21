import json
from pathlib import Path
import socket
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from server import ApiError, Gateway, IDS, Rpc, Snapshot, ThreadingHTTPServer, make_handler


# Simulated robotd responses for gateway and HTTP contract tests.
class FakeRpc:
    def __init__(self):
        self.calls, self.control_calls = [], []
        self.refuse = None
        self.control = {'owner':None,'ready':False,'generation':1,'preference':'auto'}
    def call(self, method, params=None):
        params = params or {}
        if self.refuse == method:raise ApiError('actual robotd refusal',409)
        if method.startswith('robot.control.'):
            self.control_calls.append((method,params))
            if method=='robot.control.select':
                self.control.update(preference=params['preference'],owner='web' if params['preference']=='web' else None,ready=params['preference']=='web')
            return dict(self.control)
        self.calls.append((method,params))
        return {'accepted':True}

class GatewayTests(unittest.TestCase):
    def setUp(self):
        self.rpc,self.now=FakeRpc(),10.0
        self.gateway=Gateway(self.rpc,Snapshot(),lambda:False,lambda:self.now)
    def request(self,route,body):return self.gateway.request(route,body,'browser')
    def claim(self):self.request('control',{'preference':'web'})
    def arm(self):
        self.claim();self.request('mode',{'mode':'policy'});self.rpc.calls.clear()
    def command(self):return {'twist':[.2,-.1,.5],'head':[.1,.2,-.3,.4],'mouth':15}
    def test_page_status_and_unclaimed_command_send_no_motion(self):
        self.gateway.status('browser')
        self.assertFalse(self.request('command',self.command())['forwarded'])
        self.assertEqual(self.rpc.calls,[])
    def test_mode_requires_explicit_control_claim(self):
        with self.assertRaises(ApiError):self.request('mode',{'mode':'policy'})
        self.assertEqual(self.rpc.calls,[])
    def test_control_claim_itself_sends_no_motion(self):
        self.claim();self.assertEqual(self.rpc.calls,[])
        self.assertEqual(self.rpc.control['owner'],'web')
    def test_default_pose_uses_enable_and_init(self):
        self.claim();self.request('mode',{'mode':'hold'})
        self.assertEqual([c[0] for c in self.rpc.calls],['robot.xgo.command','robot.enable','robot.init'])
        self.assertFalse(self.rpc.calls[1][1]['on'])
    def test_walk_uses_existing_enable(self):
        self.claim();self.request('mode',{'mode':'policy'})
        self.assertEqual(self.rpc.calls[-1],('robot.enable',{'on':True,'toggle':False}))
    def test_command_units_and_head_order(self):
        self.arm();self.request('command',self.command())
        self.assertEqual(self.rpc.calls,[('robot.xgo.command',{'twist':[.2,-.1,.5],'head':[.1,.2,-.3,.4],'mouth':.5})])
    def test_late_command_after_off_is_not_forwarded(self):
        self.arm();self.request('mode',{'mode':'off'});self.rpc.calls.clear()
        self.assertFalse(self.request('command',self.command())['forwarded']);self.assertEqual(self.rpc.calls,[])
    def test_release_disconnects_presence_without_direct_torque_command(self):
        self.arm();self.request('release',{})
        self.assertEqual(self.rpc.calls,[]);self.assertIsNone(self.gateway.owner)
        self.assertFalse(self.rpc.control_calls[-1][1]['connected'])
    def test_timeout_delegates_disconnect_to_robotd(self):
        self.arm();self.now+=1.1;self.gateway.expire()
        self.assertEqual(self.rpc.calls,[]);self.assertIsNone(self.gateway.session)
        self.assertFalse(self.rpc.control_calls[-1][1]['connected'])
    def test_status_does_not_refresh_presence(self):
        self.arm();last=self.gateway.session_at;self.now+=.5;self.gateway.status('browser')
        self.assertEqual(self.gateway.session_at,last)
    def test_window_background_native_pong_preserves_policy_and_zeroes_stale_velocity(self):
        self.arm();token=object();self.gateway.open_connection('browser',token)
        self.request('command',self.command());self.rpc.calls.clear();self.now+=.3
        self.assertTrue(self.gateway.connection_alive('browser',token))
        self.assertEqual(self.rpc.calls,[('robot.xgo.command',{'twist':[0.0]*3,'head':[.1,.2,-.3,.4],'mouth':.5})])
        self.assertNotIn(('robot.relax',{}),self.rpc.calls)
    def test_stale_connection_token_cannot_release_new_connection(self):
        self.arm();old,new=object(),object();self.gateway.open_connection('browser',old);self.gateway.open_connection('browser',new)
        self.gateway.close_connection('browser',old);self.assertIsNotNone(self.gateway.session)
    def test_generation_change_revokes_local_owner(self):
        self.arm();self.rpc.control['generation']+=1;self.gateway.status('browser')
        self.assertIsNone(self.gateway.owner)
    def test_pick_routes_to_existing_skill(self):
        self.arm();self.request('pick',{});self.assertEqual(self.rpc.calls,[('robot.do',{'skill':'ground_pick'})])
    def test_invalid_values_rejected_before_motion(self):
        self.arm()
        for value in [float('nan'),float('inf'),.5,True,'0.2']:
            cmd=self.command();cmd['twist'][0]=value
            with self.assertRaises(ApiError):self.request('command',cmd)
        cmd=self.command();cmd['mouth']=-1
        with self.assertRaises(ApiError):self.request('command',cmd)
        self.assertEqual(self.rpc.calls,[])
    def test_rpc_refusal_is_not_success_and_does_not_retry_action(self):
        self.claim();self.rpc.refuse='robot.enable'
        with self.assertRaises(ApiError):self.request('mode',{'mode':'policy'})
        self.assertIsNone(self.gateway.owner)
    def test_off_is_available_without_claim(self):
        self.request('mode',{'mode':'off'});self.assertEqual(self.rpc.calls,[('robot.relax',{})])
    def test_status_does_not_invent_per_servo_torque(self):
        self.gateway.snapshot.update({'policy':'held','joints':[0]*15,'targets':[0]*15,'safety':{'gravity':[0,0,-1]},'loop':{'hz':50}})
        state=self.gateway.status('browser');self.assertEqual(state['motors'],IDS)
        self.assertIsNone(state['enabled']);self.assertIsNone(state['servo_ids'])


class HttpTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rpc = FakeRpc()
        cls.gateway = Gateway(cls.rpc, Snapshot(), lambda: False)
        cls.http = ThreadingHTTPServer(('127.0.0.1', 0), make_handler(cls.gateway))
        cls.url = f'http://127.0.0.1:{cls.http.server_port}'
        cls.thread = threading.Thread(target=cls.http.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.http.shutdown()
        cls.http.server_close()
        cls.thread.join()

    def setUp(self):
        self.rpc.calls.clear()
        self.gateway.owner = None

    def post(self, path, body, **headers):
        request = urllib.request.Request(self.url + path, data=json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json', **headers})
        return urllib.request.urlopen(request)

    def test_loading_all_page_assets_and_status_sends_no_action(self):
        for path in ('/', '/app.js', '/style.css', '/api/status'):
            with urllib.request.urlopen(self.url + path) as response:
                self.assertEqual(response.status, 200)
                self.assertGreater(len(response.read()), 0)
        self.assertEqual(self.rpc.calls, [])

    def test_original_beacon_off_has_no_custom_header_requirement(self):
        with self.post('/api/mode', {'mode': 'off'}) as response:
            self.assertEqual(response.status, 200)
        self.assertEqual(self.rpc.calls, [('robot.relax', {})])

    def test_cross_site_motion_rejected(self):
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.post('/api/mode', {'mode': 'policy'}, Origin='http://other-host')
        self.assertEqual(error.exception.code, 403)
        self.assertEqual(self.rpc.calls, [])

    def test_no_arbitrary_rpc_or_calibration_endpoints(self):
        for path in ('/api/cal/enter', '/api/servo', '/api/rpc', '/api/filter'):
            with self.assertRaises(urllib.error.HTTPError) as error:
                self.post(path, {})
            self.assertEqual(error.exception.code, 404)
        self.assertEqual(self.rpc.calls, [])


@unittest.skipUnless(hasattr(socket, 'AF_UNIX'), 'Unix sockets unavailable')
class WireTests(unittest.TestCase):
    def test_fragmented_newline_reply_and_rpc_refusal(self):
        with tempfile.TemporaryDirectory() as root:
            path = str(Path(root) / 'rpc.sock')
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
                listener.bind(path)
                listener.listen()
                observed = []

                def service():
                    with listener.accept()[0] as sock, sock.makefile('rb') as stream:
                        for number in range(2):
                            call = json.loads(stream.readline())
                            observed.append(call)
                            result = {'accepted': number == 0, 'reason': 'test refusal'}
                            line = (json.dumps({'jsonrpc': '2.0', 'id': call['id'], 'result': result}) + '\n').encode()
                            sock.sendall(line[:5])
                            sock.sendall(line[5:])
                worker = threading.Thread(target=service)
                worker.start()
                rpc = Rpc(path)
                try:
                    self.assertTrue(rpc.call('robot.health')['accepted'])
                    with self.assertRaises(ApiError):
                        rpc.call('robot.enable', {'on': True, 'toggle': False})
                finally:
                    rpc.close()
                    worker.join(timeout=3)
                self.assertEqual(len(observed), 2)
                self.assertEqual(observed[1]['params'], {'on': True, 'toggle': False})


if __name__ == '__main__':
    unittest.main()
