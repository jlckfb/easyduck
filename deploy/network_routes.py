"""Read-only network addresses shared by the three local web services."""
import ipaddress
import subprocess
from pathlib import Path


def addresses():
    try:
        result = subprocess.run(['nmcli', '-t', '-f', 'DEVICE,TYPE,STATE', 'device'],
                                capture_output=True, text=True, timeout=2, check=True)
        for line in result.stdout.splitlines():
            fields = line.split(':')
            if len(fields) != 3 or fields[1:] != ['wifi', 'connected']:
                continue
            result = subprocess.run(['nmcli', '-g', 'IP4.ADDRESS', 'device', 'show', fields[0]],
                                    capture_output=True, text=True, timeout=2, check=True)
            for address in result.stdout.splitlines():
                ip = ipaddress.ip_interface(address).ip
                if ip.version == 4 and not (ip.is_loopback or ip.is_link_local or ip.is_unspecified):
                    return {'wifi_host': str(ip)}
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    return {'wifi_host': ''}


def client_script():
    return Path(__file__).with_name('navigation-client.js').read_bytes()
