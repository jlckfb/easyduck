"""NetworkManager and BlueZ operations. Never opens robot or input device streams."""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid


class SetupError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def run(args, timeout=8):
    try:
        p = subprocess.run(args, capture_output=True, text=True, timeout=timeout,
                           env={**os.environ, 'LC_ALL': 'C'})
        return p.returncode, p.stdout.strip(), p.stderr.strip()
    except subprocess.TimeoutExpired:
        return 124, '', ''
    except OSError:
        return 127, '', ''


def nm_fields(line):
    fields, current, escaped = [], '', False
    for char in line:
        if escaped:
            current += char
            escaped = False
        elif char == '\\':
            escaped = True
        elif char == ':':
            fields.append(current)
            current = ''
        else:
            current += char
    fields.append(current)
    return fields


def number(value, default=0):
    match = re.match(r'\s*(\d+)', str(value))
    return int(match.group(1)) if match else default


def valid_mac(value):
    if not re.fullmatch(r'(?:[\dA-Fa-f]{2}:){5}[\dA-Fa-f]{2}', value):
        raise SetupError('invalid_device', '请选择列表中的 PS5 手柄。')
    return value.upper()


class Backend:
    def __init__(self, state_dir='/var/lib/easyduck/setup', profiles='/etc/NetworkManager/system-connections'):
        self.state_dir = Path(state_dir)
        self.profiles = Path(profiles)

    @contextmanager
    def operation(self, resource="connections"):
        import fcntl
        if resource not in ("connections", "wifi", "bluetooth"):
            raise ValueError("invalid operation resource")
        self.state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        with (self.state_dir / (resource + '.lock')).open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise SetupError('busy', '正在处理其他连接操作，请稍后再试。')
            try:
                yield
            finally:
                fcntl.flock(lock, fcntl.LOCK_UN)

    def wifi_device(self):
        code, output, _ = run(['nmcli', '-t', '-f', 'DEVICE,TYPE,STATE', 'device'])
        if code:
            raise SetupError('network_manager', '无法读取 NetworkManager，请检查服务是否运行。')
        devices = [nm_fields(line) for line in output.splitlines()]
        devices = [d for d in devices if len(d) >= 3 and d[1] == 'wifi' and d[2] not in ('unmanaged', 'unavailable')]
        devices.sort(key=lambda d: d[2] != 'connected')
        if not devices:
            raise SetupError('no_wifi', '没有可用的 Wi-Fi 网卡，请检查网卡及系统驱动。')
        return devices[0][0]

    def wifi_networks(self, device):
        _, output, _ = run(['nmcli', '-t', '-f', 'IN-USE,SSID,SIGNAL,SECURITY',
                            'device', 'wifi', 'list', 'ifname', device, '--rescan', 'no'])
        networks = {}
        for line in output.splitlines():
            fields = nm_fields(line)
            if len(fields) != 4 or not fields[1]:
                continue
            active, ssid, signal, security = fields
            item = {'ssid': ssid, 'signal': min(100, number(signal)),
                    'security': security if security not in ('', '--') else '开放', 'active': active == '*'}
            if ssid not in networks or item['signal'] > networks[ssid]['signal']:
                networks[ssid] = item
        return sorted(networks.values(), key=lambda n: (not n['active'], -n['signal'], n['ssid']))

    def wifi_status(self):
        try:
            device = self.wifi_device()
        except SetupError as error:
            return {'device': None, 'ssid': '', 'ip': '', 'uuid': '', 'connected': False,
                    'networks': [], 'state': 0, 'reason': 0, 'error': str(error)}
        _, output, _ = run(['nmcli', '-t', '-f', 'GENERAL.STATE,GENERAL.REASON,GENERAL.CON-UUID,IP4.ADDRESS',
                            'device', 'show', device])
        info = {}
        for line in output.splitlines():
            fields = nm_fields(line)
            if len(fields) >= 2:
                info[fields[0]] = ':'.join(fields[1:])
        active_uuid = info.get('GENERAL.CON-UUID', '')
        ssid = ''
        if re.fullmatch(r'[a-fA-F0-9-]{36}', active_uuid):
            _, ssid, _ = run(['nmcli', '--escape', 'no', '-g', '802-11-wireless.ssid',
                              'connection', 'show', 'uuid', active_uuid])
        ip = info.get('IP4.ADDRESS[1]', '').split('/')[0]
        state = number(info.get('GENERAL.STATE'))
        return {'device': device, 'ssid': ssid, 'ip': ip, 'uuid': active_uuid,
                'state': state, 'reason': number(info.get('GENERAL.REASON')),
                'connected': state == 100 and bool(ssid) and bool(ip),
                'networks': self.wifi_networks(device), 'error': ''}

    def scan_wifi(self, **_):
        device = self.wifi_device()
        if run(['nmcli', '--wait', '12', 'device', 'wifi', 'rescan', 'ifname', device], 15)[0]:
            raise SetupError('scan_failed', 'Wi-Fi 扫描失败，请稍后重试并检查无线开关。')
        # NetworkManager rescan returns when scheduled, before all results arrive.
        time.sleep(1)
        return {'message': 'Wi-Fi 扫描完成。', 'wifi': self.wifi_status()}

    @staticmethod
    def wifi_failure(code, output, state, ssid, requested_uuid):
        text = output.lower()
        if state.get('reason') == 53 or 'no network with ssid' in text or 'ssid-not-found' in text:
            return SetupError('network_not_found', '没有找到该网络，请确认热点已开启、信号可达后重新扫描。')
        target = state.get('uuid') == requested_uuid and state.get('ssid') == ssid
        dhcp_failed = ('ip configuration could not be reserved' in text or
                       (state.get('reason') in (5, 15, 16, 17) and state.get('state') != 100))
        if not state.get('ip') and (dhcp_failed or (target and state.get('state') in (70, 80, 90))):
            return SetupError('dhcp_timeout', '已关联目标网络，但没有取得 IP（DHCP 无响应），请检查路由器或热点的 DHCP。')
        if state.get('reason') == 7 or any(word in text for word in ('secrets were required', 'wrong key', 'pre-shared key', 'no secrets', 'authentication failed')):
            return SetupError('wrong_password', 'Wi-Fi 密码错误或认证未通过，请重新输入密码。')
        if code == 124 or state.get('reason') == 11:
            return SetupError('wifi_timeout', 'Wi-Fi 认证或连接超时，请核对密码、信号和热点加密方式。')
        return SetupError('wifi_failed', '目标网络未激活或未取得 IP，请核对密码和信号后重试。')

    def connect_wifi(self, ssid='', password='', **_):
        if not isinstance(ssid, str) or not 1 <= len(ssid.encode()) <= 32 or any(c in ssid for c in '\x00\r\n'):
            raise SetupError('invalid_ssid', 'SSID 长度应为 1～32 字节。')
        if not isinstance(password, str) or any(c in password for c in '\x00\r\n'):
            raise SetupError('invalid_password', '密码包含不支持的字符。')
        device = self.wifi_device()
        network = next((n for n in self.wifi_networks(device) if n['ssid'] == ssid), None)
        if network is None:
            self.scan_wifi()
            network = next((n for n in self.wifi_networks(device) if n['ssid'] == ssid), None)
        if network is None:
            raise SetupError('network_not_found', '没有找到该网络，请开启热点后重新扫描。')
        security = network['security']
        if '802.1X' in security or 'WEP' in security:
            raise SetupError('unsupported_security', '该网络需要企业认证或旧式 WEP，请使用个人 WPA2/WPA3 或开放网络。')
        if security != '开放' and not (8 <= len(password.encode()) <= 63 or re.fullmatch('[0-9a-fA-F]{64}', password)):
            raise SetupError('wrong_password', '此网络需要有效的 Wi-Fi 密码（通常为 8～63 字节）。')
        requested_uuid = str(uuid.uuid4())
        self.profiles.mkdir(parents=True, exist_ok=True)
        path = self.profiles / ('easyduck-' + requested_uuid + '.nmconnection')
        # Native NetworkManager keyfile: root-only secret, never in argv or logs.
        ssid_bytes = ';'.join(str(c) for c in ssid.encode()) + ';'
        contents = f'[connection]\nid=easyduck-{requested_uuid[:8]}\nuuid={requested_uuid}\ntype=wifi\ninterface-name={device}\nautoconnect=false\n\n[wifi]\nmode=infrastructure\nssid={ssid_bytes}\n'
        if security != '开放':
            key = password.replace('\\', '\\\\').replace(' ', '\\s').replace('\t', '\\t')
            management = 'sae' if 'WPA3' in security and 'WPA2' not in security else 'wpa-psk'
            contents += f'\n[wifi-security]\nkey-mgmt={management}\npsk={key}\n'
        contents += '\n[ipv4]\nmethod=auto\n\n[ipv6]\nmethod=auto\n'
        keep = False
        try:
            with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w', encoding='utf-8') as stream:
                stream.write(contents)
            if run(['nmcli', 'connection', 'load', str(path)])[0]:
                raise SetupError('profile_failed', 'NetworkManager 未能载入连接配置。')
            code, out, err = run(['nmcli', '--wait', '65', 'connection', 'up', 'uuid', requested_uuid, 'ifname', device], 70)
            state = self.wifi_status()
            if not (code == 0 and state['connected'] and state['uuid'] == requested_uuid and state['ssid'] == ssid):
                raise self.wifi_failure(code, out + '\n' + err, state, ssid, requested_uuid)
            if run(['nmcli', 'connection', 'modify', 'uuid', requested_uuid, 'connection.autoconnect', 'yes'])[0]:
                raise SetupError('save_failed', '已连接，但自动重连设置未保存，请重试。')
            keep = True
            return {'message': f'已连接 {ssid}，IP：{state["ip"]}。', 'wifi': state}
        finally:
            if not keep:
                run(['nmcli', 'connection', 'delete', 'uuid', requested_uuid])
                path.unlink(missing_ok=True)

    def adapter(self):
        code, output, _ = run(['bluetoothctl', 'show'])
        found = not code and re.search(r'Controller ([\dA-Fa-f:]{17})', output)
        return {'available': bool(found), 'powered': 'Powered: yes' in output,
                'name': (re.search(r'^\s*Name: (.*)', output, re.M).group(1)
                         if re.search(r'^\s*Name: (.*)', output, re.M) else '')}

    def require_adapter(self):
        adapter = self.adapter()
        if not adapter['available']:
            raise SetupError('no_adapter', '没有蓝牙适配器，请检查蓝牙服务和硬件。')
        if not adapter['powered'] and run(['bluetoothctl', 'power', 'on'])[0]:
            raise SetupError('adapter_off', '蓝牙适配器未开启，请检查系统无线开关。')

    @staticmethod
    def battery(address, info, power_supply=Path('/sys/class/power_supply')):
        match = re.search(r'Battery Percentage:\s*(0x[\da-fA-F]+|\d+)', info)
        if match:
            value = int(match.group(1), 0) if match.group(1).startswith('0x') else int(match.group(1))
            if 0 <= value <= 100:
                return value
        normalized = address.lower().replace(':', '')
        for supply in power_supply.glob('*'):
            try:
                metadata = (supply / 'uevent').read_text() + str(supply.resolve())
                if normalized not in re.sub('[^a-z0-9]', '', metadata.lower()):
                    continue
                value = int((supply / 'capacity').read_text())
                if 0 <= value <= 100:
                    return value
            except (OSError, ValueError):
                continue
        return None

    @staticmethod
    def input_nodes(address, source=Path('/proc/bus/input/devices')):
        try:
            blocks = source.read_text().split('\n\n')
        except OSError:
            return []
        nodes = []
        for block in blocks:
            if address.lower() not in block.lower():
                continue
            match = re.search(r'H: Handlers=(.*)', block)
            if match:
                nodes.extend('/dev/input/' + n for n in match.group(1).split() if re.fullmatch(r'(?:js|event)\d+', n))
        return nodes

    def devices(self):
        _, output, _ = run(['bluetoothctl', 'devices'])
        enrolled = self.enrolled()
        devices = []
        for line in output.splitlines():
            match = re.fullmatch(r'Device ([\dA-Fa-f:]{17}) (.*)', line)
            if not match:
                continue
            address, name = match.groups()
            address = address.upper()
            # Display PS5-compatible names and devices previously enrolled here.
            if not any(n in name.lower() for n in ('wireless controller', 'dualsense')) and address not in enrolled:
                continue
            _, info, _ = run(['bluetoothctl', 'info', address])
            devices.append({'address': address, 'name': name, 'paired': 'Paired: yes' in info,
                            'trusted': 'Trusted: yes' in info, 'connected': 'Connected: yes' in info,
                            'battery': self.battery(address, info), 'inputs': self.input_nodes(address),
                            'auto_reconnect': address in enrolled})
        return sorted(devices, key=lambda d: (not d['connected'], not d['paired'], d['name']))

    def bluetooth_status(self):
        adapter = self.adapter()
        return {'adapter': adapter, 'devices': self.devices() if adapter['available'] else []}

    def scan_bluetooth(self, **_):
        self.require_adapter()
        code, output, err = run(['bluetoothctl', '--timeout', '8', 'scan', 'on'], 12)
        run(['bluetoothctl', 'scan', 'off'])
        if code not in (0, 124) or 'Failed to start discovery' in output + err:
            raise SetupError('bluetooth_scan_failed', '蓝牙扫描失败，请检查适配器后重试。')
        return {'message': '扫描结束。未出现手柄时，请同时按住 PS 键和 Create 键，直到灯条闪烁。',
                'bluetooth': self.bluetooth_status()}

    def enrolled(self):
        try:
            data = json.loads((self.state_dir / 'gamepads.json').read_text())
            return data if isinstance(data, dict) else {}
        except (OSError, ValueError):
            return {}

    def save_enrolled(self, data):
        self.state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        temporary = self.state_dir / 'gamepads.json.new'
        with os.fdopen(os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), 'w') as stream:
            json.dump(data, stream)
        os.replace(temporary, self.state_dir / 'gamepads.json')

    @staticmethod
    def boot_id():
        try:
            return Path('/proc/sys/kernel/random/boot_id').read_text().strip()
        except OSError:
            return 'unknown-boot'

    def device(self, address):
        address = valid_mac(address)
        device = next((d for d in self.devices() if d['address'] == address), None)
        if not device:
            raise SetupError('not_discovered', '未发现该手柄。请同时按住 PS 和 Create 键，灯条闪烁后重新扫描。')
        return device

    @staticmethod
    def bluetooth_failure(code, output, pairing=False):
        text = output.lower()
        if code == 124 or any(w in text for w in ('authenticationtimeout', 'page-timeout', 'connection-timeout')):
            return SetupError('pair_timeout' if pairing else 'connect_timeout', '配对超时，请重新进入配对模式。' if pairing else '连接超时，请打开手柄并确认它未连接其他主机。')
        if any(w in text for w in ('authenticationfailed', 'authenticationrejected', 'authenticationcanceled')):
            return SetupError('pair_failed', '配对被拒绝或认证失败。请断开其他主机上的手柄连接，再进入配对模式。')
        if 'inprogress' in text or 'alreadyconnected' in text:
            return SetupError('device_busy', '设备正在连接或被占用，请断开其他主机连接后重试。')
        return SetupError('not_pairing' if pairing else 'connect_failed',
                          '手柄未响应配对，请按住 PS 和 Create 键直到灯条闪烁；若仍失败，请检查是否已连接其他主机。' if pairing
                          else '手柄连接失败，可能未开启、超出范围或已连接其他主机，请核对后重试。')

    def gamepad(self, action, address='', **_):
        self.require_adapter()
        device = self.device(address)
        address = device['address']
        if action == 'pair':
            if not device['paired']:
                code, out, err = run(['bluetoothctl', '--timeout', '25', '--agent', 'NoInputNoOutput', 'pair', address], 30)
                if code or not self.device(address)['paired']:
                    raise self.bluetooth_failure(code, out + err, pairing=True)
            if run(['bluetoothctl', 'trust', address])[0] or not self.device(address)['trusted']:
                raise SetupError('trust_failed', '配对成功，但保存信任状态失败，请重试。')
            enrolled = self.enrolled()
            enrolled[address] = {'pause_boot': ''}
            self.save_enrolled(enrolled)
            action = 'connect'
        enrolled = self.enrolled()
        if action == 'connect':
            if not device['paired'] and not self.device(address)['paired']:
                raise SetupError('not_paired', '请先配对该手柄。')
            code, out, err = run(['bluetoothctl', '--timeout', '12', 'connect', address], 16)
            fresh = self.device(address)
            if not fresh['connected']:
                raise self.bluetooth_failure(code, out + err)
            if not fresh['trusted']:
                if run(['bluetoothctl', 'trust', address])[0] or not self.device(address)['trusted']:
                    raise SetupError('trust_failed', '已连接，但信任状态未保存，请重新配对。')
            enrolled[address] = {'pause_boot': ''}
            self.save_enrolled(enrolled)
            message = '手柄已连接，自动重连已启用。'
        elif action == 'disconnect':
            # Save pause first so the timer cannot immediately undo a manual disconnect.
            if address in enrolled:
                enrolled[address]['pause_boot'] = self.boot_id()
                self.save_enrolled(enrolled)
            code, out, err = run(['bluetoothctl', '--timeout', '8', 'disconnect', address], 12)
            if self.device(address)['connected']:
                raise SetupError('disconnect_failed', '手柄仍处于连接状态，请重试。')
            message = '手柄已断开；本次开机期间暂停自动重连，点击连接可恢复。'
        elif action == 'forget':
            if run(['bluetoothctl', 'remove', address])[0]:
                raise SetupError('forget_failed', '忘记设备失败，请稍后重试。')
            if any(d['address'] == address and d['paired'] for d in self.devices()):
                raise SetupError('forget_failed', '系统仍保留配对信息，请稍后重试。')
            enrolled.pop(address, None)
            self.save_enrolled(enrolled)
            message = '已忘记该手柄，之后需要重新配对。'
        else:
            raise SetupError('invalid_action', '不支持的手柄操作。')
        return {'message': message, 'bluetooth': self.bluetooth_status()}

    def reconnect_once(self):
        if not self.adapter()['available']:
            return
        for device in self.devices():
            entry = self.enrolled().get(device['address'])
            if not entry or entry.get('pause_boot') == self.boot_id() or device['connected'] or not (device['paired'] and device['trusted']):
                continue
            self.gamepad('connect', address=device['address'])
            break

    def status(self):
        access = {}
        try:
            access = json.loads(Path('/run/easyduck-network/status.json').read_text())
            if not isinstance(access, dict): access = {}
        except (OSError, ValueError):
            pass
        return {'wifi': self.wifi_status(), 'bluetooth': self.bluetooth_status(),
                'hostname': os.uname().nodename, 'network_access': access}
