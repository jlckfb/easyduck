import contextlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).parents[1]/'setup'))
import connections as c
import first_setup as web

@unittest.skipUnless(os.name == 'posix', 'Linux advisory locks')
class ConnectionLockTests(unittest.TestCase):
    def test_offline_pad_reconnect_cannot_block_wifi(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend=c.Backend(tmp)
            with backend.operation('bluetooth'):
                with backend.operation('wifi'):
                    pass
                with self.assertRaises(c.SetupError) as error:
                    with backend.operation('bluetooth'):
                        pass
                self.assertEqual(error.exception.code,'busy')

class WifiTests(unittest.TestCase):
    def test_nm_escaped_ssid_and_separator(self):
        self.assertEqual(c.nm_fields(r'*:a\:b\\c:92:WPA2'),['*','a:b\\c','92','WPA2'])
    def test_unmanaged_radio_not_selected(self):
        with patch.object(c,'run',return_value=(0,'wlan0:wifi:unmanaged\nwlan9:wifi:disconnected','')):
            self.assertEqual(c.Backend().wifi_device(),'wlan9')
    def test_wifi_status_uses_active_profile_ssid_not_connection_name(self):
        uid='a'*8+'-'+ 'b'*4+'-'+ 'c'*4+'-'+ 'd'*4+'-'+ 'e'*12
        def run(args,*_):
            if 'DEVICE,TYPE,STATE' in args:return 0,'wifi9:wifi:connected',''
            if 'device' in args and 'show' in args:return 0,f'GENERAL.STATE:100 (connected)\nGENERAL.REASON:0 (none)\nGENERAL.CON-UUID:{uid}\nIP4.ADDRESS[1]:10.20.30.4/24',''
            if '802-11-wireless.ssid' in args:return 0,'target:ssid',''
            return 0,'*:target\\:ssid:90:WPA2',''
        with patch.object(c,'run',side_effect=run):
            status=c.Backend().wifi_status()
        self.assertEqual(status['ssid'],'target:ssid');self.assertTrue(status['connected']);self.assertEqual(status['ip'],'10.20.30.4')
    def activate(self,mode='success'):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        backend=c.Backend(Path(temp.name)/'state',Path(temp.name)/'profiles')
        backend.wifi_device=lambda:'wifi9'
        backend.wifi_networks=lambda dev:[{'ssid':'target:ssid','security':'WPA2','signal':90}]
        calls=[];target={};snapshots=[]
        def run(args,*_):
            calls.append(args)
            if 'load' in args:
                profile=Path(args[-1]);text=profile.read_text();target['uuid']=re.search(r'^uuid=(.*)$',text,re.M).group(1)
                snapshots.append((text,stat.S_IMODE(profile.stat().st_mode)))
            if 'up' in args:
                return (1,'','Secrets were required, but not provided') if mode=='password' else (0,'','')
            return 0,'',''
        def status():
            return {'device':'wifi9','connected':mode not in ('password','dhcp'),'ssid':'old' if mode=='old' else 'target:ssid','uuid':'old-uuid' if mode=='old' else target['uuid'],'ip':'' if mode in ('password','dhcp') else '10.20.30.4','state':70 if mode=='dhcp' else 100,'reason':7 if mode=='password' else 0}
        backend.wifi_status=status
        with patch.object(c,'run',side_effect=run):
            try:result=backend.connect_wifi('target:ssid','test-only-password')
            except c.SetupError as error:result=error
        return backend,calls,snapshots,result
    def test_success_confirms_uuid_and_saves_secret_without_argv(self):
        backend,calls,snapshots,result=self.activate()
        self.assertIsInstance(result,dict)
        self.assertTrue(any('connection.autoconnect' in args for args in calls))
        self.assertFalse(any('test-only-password' in arg for args in calls for arg in args))
        self.assertIn('psk=test-only-password',snapshots[0][0])
        if os.name=='posix':self.assertEqual(snapshots[0][1],0o600)
    def test_old_connected_network_cannot_report_target_success(self):
        backend,calls,_,result=self.activate('old')
        self.assertIsInstance(result,c.SetupError)
        self.assertFalse(list(backend.profiles.glob('*.nmconnection')))
        self.assertFalse(any('connection.autoconnect' in args for args in calls))
    def test_password_error_is_explicit_and_failed_profile_removed(self):
        backend,_,_,result=self.activate('password')
        self.assertEqual(result.code,'wrong_password');self.assertFalse(list(backend.profiles.glob('*')))
    def test_target_associated_without_ip_is_dhcp_error(self):
        _,_,_,result=self.activate('dhcp');self.assertEqual(result.code,'dhcp_timeout')
    def test_dhcp_error_survives_nm_state_reset(self):
        state={'state':30,'reason':0,'uuid':'','ssid':'','ip':''}
        error=c.Backend.wifi_failure(4,'Error: Connection activation failed: IP configuration could not be reserved (no available address, timeout, etc.)',state,'target','requested')
        self.assertEqual(error.code,'dhcp_timeout')
    def test_missing_network_is_not_password_error(self):
        b=c.Backend();b.wifi_device=lambda:'wifi9';b.wifi_networks=lambda _:[];b.scan_wifi=lambda:None
        with self.assertRaises(c.SetupError) as error:b.connect_wifi('missing','test-only-password')
        self.assertEqual(error.exception.code,'network_not_found')

class BluetoothTests(unittest.TestCase):
    def test_battery_bluez_and_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(c.Backend.battery('AA:BB:CC:DD:EE:FF','Battery Percentage: 0x64 (100)',Path(tmp)),100)
            self.assertIsNone(c.Backend.battery('AA:BB:CC:DD:EE:FF','',Path(tmp)))
    def test_battery_sysfs_matches_device_mac(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'pad';p.mkdir();(p/'uevent').write_text('POWER_SUPPLY_SERIAL_NUMBER=aa:bb:cc:dd:ee:ff');(p/'capacity').write_text('76')
            self.assertEqual(c.Backend.battery('AA:BB:CC:DD:EE:FF','',Path(tmp)),76)
            self.assertIsNone(c.Backend.battery('11:22:33:44:55:66','',Path(tmp)))
    def test_input_nodes_only_read_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'devices';p.write_text('N: Name="Wireless Controller"\nU: Uniq=aa:bb:cc:dd:ee:ff\nH: Handlers=event4 js0\n')
            self.assertEqual(c.Backend.input_nodes('AA:BB:CC:DD:EE:FF',p),['/dev/input/event4','/dev/input/js0'])
    def make_backend(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        b=c.Backend(temp.name);b.adapter=lambda:{'available':True,'powered':True}
        d={'address':'AA:BB:CC:DD:EE:FF','name':'DualSense Wireless Controller','paired':False,'trusted':False,'connected':False}
        b.devices=lambda:[dict(d)]
        calls=[]
        def run(args,*_):
            calls.append(args)
            if 'pair' in args:d['paired']=True
            elif 'trust' in args:d['trusted']=True
            elif 'disconnect' in args:d['connected']=False
            elif 'connect' in args:d['connected']=True
            elif 'remove' in args:b.devices=lambda:[]
            return 0,'',''
        return b,d,calls,run
    def test_pair_trust_connect_disconnect_reconnect_forget(self):
        b,d,calls,run=self.make_backend()
        with patch.object(c,'run',side_effect=run):
            b.gamepad('pair',address=d['address'])
            self.assertTrue(d['connected']);self.assertIn(d['address'],b.enrolled())
            self.assertEqual([next(x for x in ('pair','trust','connect') if x in a) for a in calls],['pair','trust','connect'])
            b.gamepad('disconnect',address=d['address']);count=len(calls);b.reconnect_once();self.assertEqual(len(calls),count)
            with patch.object(b,'boot_id',return_value='new-boot'):b.reconnect_once()
            self.assertTrue(d['connected'])
            b.gamepad('forget',address=d['address']);self.assertFalse(b.enrolled())
    def test_no_adapter_and_invalid_address_do_not_run_mutations(self):
        b=c.Backend();b.adapter=lambda:{'available':False,'powered':False}
        with patch.object(c,'run') as run:
            with self.assertRaises(c.SetupError) as error:b.gamepad('pair',address='AA:BB:CC:DD:EE:FF')
            self.assertEqual(error.exception.code,'no_adapter');run.assert_not_called()
        with self.assertRaises(c.SetupError):c.valid_mac('AA; unexpected')
    def test_pair_timeout_and_busy_have_distinct_messages(self):
        self.assertEqual(c.Backend.bluetooth_failure(124,'',True).code,'pair_timeout')
        self.assertEqual(c.Backend.bluetooth_failure(1,'org.bluez.Error.InProgress').code,'device_busy')

class FakeBackend:
    def operation(self, resource="connections"):return contextlib.nullcontext()
    def status(self):return {'hostname':'test-device','wifi':{'ssid':'test','ip':'10.20.30.4','connected':True,'networks':[]},'bluetooth':{'adapter':{'available':True,'powered':True},'devices':[]}}
    def scan_wifi(self):return {'message':'扫描完成','wifi':self.status()['wifi']}
    def connect_wifi(self,**body):raise c.SetupError('wrong_password','密码错误')
class FakeAuth:
    username='test-user'
    def verify(self,user,password):return user=='test-user' and password=='test-only-password'

class HttpTests(unittest.TestCase):
    def setUp(self):
        self.app=web.App(FakeBackend(),FakeAuth())
        self.server=web.ThreadingHTTPServer(('127.0.0.1',0),web.make_handler(self.app))
        self.thread=threading.Thread(target=self.server.serve_forever,daemon=True);self.thread.start()
        self.addCleanup(self.server.server_close);self.addCleanup(self.thread.join);self.addCleanup(self.server.shutdown)
    def request(self,method,path,body=None,headers=None):
        conn=http.client.HTTPConnection(*self.server.server_address,timeout=5)
        hdr={'Content-Type':'application/json',**(headers or {})}
        conn.request(method,path,json.dumps(body) if body is not None else None,hdr)
        response=conn.getresponse();data=response.read();result=(response.status,dict(response.getheaders()),data)
        conn.close();return result
    def login(self):
        code,headers,_=self.request('POST','/api/login',{'username':'test-user','password':'test-only-password'})
        self.assertEqual(code,200);cookie=headers['Set-Cookie'].split(';')[0]
        code,_,page=self.request('GET','/',headers={'Cookie':cookie})
        csrf=re.search(r'name="csrf-token" content="([^"]+)"',page.decode()).group(1)
        return {'Cookie':cookie,'X-CSRF-Token':csrf}
    def test_login_reuses_system_auth_contract_and_sets_httponly(self):
        headers=self.login();self.assertIn('easyduck_setup=',headers['Cookie'])
    def test_unauthed_api_and_missing_csrf_cannot_mutate(self):
        self.assertEqual(self.request('POST','/api/wifi/scan',{})[0],401)
        headers=self.login();headers.pop('X-CSRF-Token');self.assertEqual(self.request('POST','/api/wifi/scan',{},headers)[0],403)
    def test_wrong_origin_rejected(self):
        headers=self.login();headers['Origin']='http://untrusted.invalid';headers['Host']='127.0.0.1'
        self.assertEqual(self.request('POST','/api/wifi/scan',{},headers)[0],403)
    def test_job_failure_keeps_page_and_returns_explicit_error(self):
        headers=self.login();code,_,data=self.request('POST','/api/wifi/connect',{'ssid':'test','password':'wrong'},headers)
        self.assertEqual(code,202);job=json.loads(data)['job']
        for _ in range(30):
            _,_,data=self.request('GET','/api/jobs/'+job,headers=headers);result=json.loads(data)
            if result['done']:break
            time.sleep(.01)
        self.assertFalse(result['result']['ok']);self.assertEqual(result['result']['code'],'wrong_password')
    def test_successful_scan_is_asynchronous(self):
        headers=self.login();code,_,data=self.request('POST','/api/wifi/scan',{},headers)
        self.assertEqual(code,202);self.assertIn('job',json.loads(data))
    def test_unknown_path_and_static_traversal_rejected(self):
        headers=self.login();self.assertEqual(self.request('GET','/static/../connections.py',headers=headers)[0],404)
    def test_status_does_not_use_caller_host_for_control_link(self):
        headers=self.login();headers['Host']='untrusted.invalid'
        _,_,data=self.request('GET','/api/status',headers=headers)
        self.assertEqual(json.loads(data)['data']['control_url'],'http://127.0.0.1:9527/')
    def test_locale_asset_is_public_before_login(self):
        code,headers,data=self.request('GET','/static/i18n.js?v=20261111')
        self.assertEqual(code,200)
        self.assertIn('text/javascript',headers['Content-Type'])
        self.assertIn(b'const setupLocale',data)
        code,_,page=self.request('GET','/login?lang=en')
        self.assertEqual(code,200)
        self.assertIn(b'id="setup-language"',page)

    def test_language_survives_login_redirects_and_remains_allowlisted(self):
        code,headers,_=self.request('GET','/navigate?target=control&source=setup&lang=en')
        self.assertEqual(code,302)
        self.assertEqual(headers['Location'],'/login?target=control&source=setup&lang=en')
        code,headers,_=self.request('GET','/?lang=unsupported')
        self.assertEqual(code,302)
        self.assertEqual(headers['Location'],'/login')
        authenticated=self.login()
        code,headers,_=self.request('GET','/login?target=control&lang=en',headers=authenticated)
        self.assertEqual(code,302)
        self.assertEqual(headers['Location'],'/navigate?target=control&source=setup&lang=en')

    def test_login_rate_limit(self):
        for _ in range(5):self.assertEqual(self.request('POST','/api/login',{'username':'test-user','password':'wrong'})[0],401)
        self.assertEqual(self.request('POST','/api/login',{'username':'test-user','password':'wrong'})[0],429)

if __name__=='__main__':unittest.main()
