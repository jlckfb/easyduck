"""Offline address discovery/announcement contract; no board, speaker or network changes."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('network_access', ROOT/'deploy/network-access/network_access.py')
network = importlib.util.module_from_spec(spec)
spec.loader.exec_module(network)


class AnnouncementTests(unittest.TestCase):
    def setUp(self):
        self.now = 0
        self.speak = Mock(return_value=True)
        self.announcer = network.Announcer(self.speak, lambda:self.now)

    def tick(self, address='192.168.43.7', idle=True, at=20):
        self.now = at
        return self.announcer.tick(address, idle)

    def test_waits_for_startup_and_stable_address_then_only_speaks_once(self):
        self.assertFalse(self.tick(at=0))
        self.assertFalse(self.tick(at=5))
        self.assertTrue(self.tick(at=15))
        self.assertFalse(self.tick(at=100))
        self.speak.assert_called_once_with('192.168.43.7')

    def test_ip_change_speaks_new_address_without_restarting_services(self):
        self.tick(at=0); self.tick(at=20)
        self.assertFalse(self.tick('172.20.10.2', at=21))
        self.assertTrue(self.tick('172.20.10.2', at=24))
        self.assertEqual([x.args[0] for x in self.speak.call_args_list],['192.168.43.7','172.20.10.2'])

    def test_policy_activity_defers_announcement(self):
        self.tick(at=0)
        self.assertFalse(self.tick(idle=False,at=20)); self.speak.assert_not_called()
        self.assertTrue(self.tick(idle=True,at=21))

    def test_brief_disconnect_does_not_repeat_but_a_new_connection_does(self):
        self.tick(at=0); self.tick(at=20)
        self.tick('',at=21); self.tick(at=22); self.assertFalse(self.tick(at=30))
        self.tick('',at=31); self.tick('',at=42); self.tick(at=43)
        self.assertTrue(self.tick(at=46)); self.assertEqual(self.speak.call_count,2)

    def test_audio_failure_is_not_marked_as_announced_and_retry_is_limited(self):
        self.speak.return_value=False
        self.tick(at=0); self.assertFalse(self.tick(at=20))
        self.assertEqual(self.announcer.announced,'')
        self.tick(at=21); self.assertEqual(self.speak.call_count,1)
        self.speak.return_value=True; self.assertTrue(self.tick(at=35))

    def test_ip_spoken_digit_by_digit_and_untrusted_text_is_rejected(self):
        self.assertIn('yi1 jiu3 er4 dian3 yi1 liu4 ba1 dian3 ling2 dian3 yi1',network.speech_text('192.168.0.1'))
        self.assertTrue(network.speech_text('192.168.0.1').isascii())
        self.assertEqual(network.Speaker('/unused').voice,'cmn-latn-pinyin')
        with self.assertRaises(ValueError): network.speech_text('1.2.3.4; reboot')

    def test_name_comes_from_avahi_including_collision_suffix(self):
        with patch.object(network.subprocess,'run',return_value=Mock(stdout='s "easyduck-2.local"')):
            self.assertEqual(network.mdns_name(),'easyduck-2.local')
        with patch.object(network.subprocess,'run',return_value=Mock(stdout='s "bad/name.local"')):
            self.assertIsNone(network.mdns_name())

    def test_unavailable_name_is_not_presented_as_working(self):
        with patch.object(network.subprocess,'run',side_effect=FileNotFoundError):
            self.assertIsNone(network.mdns_name())
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'status.json';network.write_status(p,'192.168.43.7',None,'')
            self.assertIsNone(json.loads(p.read_text())['control_url'])

    def test_read_only_robot_subscription_defers_to_enabled_policy(self):
        from unittest.mock import MagicMock
        sock=MagicMock();sock.__enter__.return_value=sock
        file=MagicMock();sock.makefile.return_value.__enter__.return_value=file
        with patch.object(network.Path,'exists',return_value=True),patch.object(network.socket,'AF_UNIX',1,create=True),patch.object(network.socket,'socket',return_value=sock):
            file.readline.side_effect=[b'{"id":1,"result":{}}',b'{"method":"robot.state","params":{"enabled":true,"policy":"xgo_rise"}}']
            self.assertFalse(network.robot_idle())
            sent=json.loads(sock.sendall.call_args.args[0]);self.assertEqual(sent['method'],'robot.subscribe')
            file.readline.side_effect=[b'{"method":"robot.state","params":{"enabled":false,"policy":"xgo_off"}}']
            self.assertTrue(network.robot_idle())

    def test_speech_yields_when_policy_starts_and_does_not_claim_success(self):
        player = Mock()
        player.poll.side_effect = [None,0]
        with tempfile.TemporaryDirectory() as d,patch.object(network,'render_speech'),patch.object(network.subprocess,'Popen',return_value=player),patch.object(network,'robot_idle',side_effect=[True,False]),patch.object(network,'pcm_idle',return_value=True):
            self.assertFalse(network.Speaker(d)('192.168.43.7'))
            player.terminate.assert_called_once()

    def test_busy_speaker_does_not_start_another_player(self):
        with tempfile.TemporaryDirectory() as d,patch.object(network,'render_speech'),patch.object(network.subprocess,'Popen') as start,patch.object(network,'robot_idle',return_value=True),patch.object(network,'pcm_idle',return_value=False):
            self.assertFalse(network.Speaker(d)('192.168.43.7'))
            start.assert_not_called()

    def test_pcm_contains_exact_silence_between_address_segments(self):
        import wave
        def synth(args, **kwargs):
            path = args[args.index('-w')+1]
            with wave.open(path,'wb') as f:
                f.setnchannels(1);f.setsampwidth(2);f.setframerate(1000)
                f.writeframes(b'\x01\x01'*10)
        with tempfile.TemporaryDirectory() as d,patch.object(network.subprocess,'run',side_effect=synth):
            p=Path(d)/'speech.wav';network.render_speech('192.168.137.138',p)
            with wave.open(str(p)) as f: pcm=f.readframes(f.getnframes())
            expected=b''.join(b'\x01\x01'*10+b'\0'*(2*pause) for _,pause in network.speech_segments('192.168.137.138'))
            self.assertEqual(pcm,expected)
            self.assertEqual(list(Path(d).glob('speech-*')),[])
        parts=network.speech_segments('192.168.137.138')
        self.assertEqual(parts[2],('yi1 jiu3 er4',500))
        self.assertEqual(parts[3],('dian3',350))

    def test_installation_maps_service_script_and_avahi_override(self):
        spec=importlib.util.spec_from_file_location('access_install',ROOT/'deploy/install.py')
        install=importlib.util.module_from_spec(spec);spec.loader.exec_module(install)
        paths={str(dst).replace('\\','/') for _,dst in install.mapping('board',ROOT/'build/bin')}
        for p in ['opt/robot/network-access/network_access.py','etc/avahi/easyduck.conf',
                  'etc/systemd/system/avahi-daemon.service.d/50-easyduck.conf',
                  'etc/systemd/system/easyduck-network-access.service']:
            self.assertIn(p,paths)


if __name__ == '__main__': unittest.main()
