#!/usr/bin/env bash
set -euo pipefail
task_source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
python3 - "$task_source_dir" "$@" <<'PY'
import argparse, hashlib, json, os, shutil, socket, subprocess, sys, tempfile, time
from pathlib import Path
source=Path(sys.argv[1])
parser=argparse.ArgumentParser(description='Install only the Easyduck connection setup page and reconnect timer.')
parser.add_argument('--root',default='/')
parser.add_argument('--port',type=int,default=80)
parser.add_argument('--host',default='0.0.0.0')
parser.add_argument('--user',default='')
args=parser.parse_args(sys.argv[2:])
target=Path(args.root).resolve()
if not 1<=args.port<=65535:parser.error('Invalid port')
if target==Path('/'):
    if os.geteuid()!=0:parser.error('Run as root for board installation')
    for program in ('nmcli','bluetoothctl','systemctl','avahi-daemon','espeak-ng','aplay','busctl'):
        if not shutil.which(program):parser.error('Missing system dependency: '+program)
    if subprocess.run(['systemctl','is-active','--quiet','easyduck-setup.service']).returncode:
        with socket.socket() as probe:
            try:probe.bind((args.host,args.port))
            except OSError:parser.error('Port is in use; choose --port or adjust the existing web service first')
if not args.user:args.user='lckfb'
import re
if not re.fullmatch(r'[a-zA-Z_][a-zA-Z0-9_.-]*[$]?',args.user):parser.error('Invalid login user')
if not re.fullmatch(r'[0-9.]+',args.host):parser.error('Use an IPv4 bind address')
entries=[(source/name,Path('opt/robot/setup')/name) for name in ('first_setup.py','connections.py','navigation.py')]
entries.extend((source.parent/name,Path('opt/robot/deploy')/name) for name in ('network_routes.py','navigation-client.js'))
entries.extend((p,Path('opt/robot/setup/static')/p.name) for p in sorted((source/'static').iterdir()) if p.is_file())
entries.append((source.parent/'network-access/network_access.py',Path('opt/robot/network-access/network_access.py')))
entries.append((source.parent/'network-access/avahi-daemon.conf',Path('etc/avahi/easyduck.conf')))
for name in ('easyduck-setup.service','easyduck-gamepad-reconnect.service','easyduck-gamepad-reconnect.timer','easyduck-network-access.service','avahi-daemon.service.d/50-easyduck.conf'):
    entries.append((source.parent/'systemd'/name,Path('etc/systemd/system')/name))
if any(not src.is_file() for src,_ in entries):parser.error('Installation source is incomplete')
def destination(relative):
    path=target/relative
    if path.is_symlink():raise ValueError('Refusing symlink: '+str(path))
    path.parent.resolve().relative_to(target)
    return path
env=destination(Path('etc/easyduck/setup.env'))
backup=target/'var/lib/easyduck/backups'/str(time.time_ns())
backup.mkdir(parents=True)
records=[]
payloads=[(relative,src.read_bytes()) for src,relative in entries]
if not env.exists():
    payloads.append((Path('etc/easyduck/setup.env'),f'EASYDUCK_SETUP_USER={args.user}\nEASYDUCK_SETUP_HOST={args.host}\nEASYDUCK_SETUP_PORT={args.port}\n'.encode()))
for relative,data in payloads:
    path=destination(relative)
    record={'path':relative.as_posix(),'existed':path.exists(),'sha256':hashlib.sha256(data).hexdigest()}
    if path.exists():
        saved=backup/'files'/relative;saved.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,saved)
        record['backup_sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
    records.append(record)
(backup/'manifest.json').write_text(json.dumps({'files':records},indent=2)+'\n')
for relative,data in payloads:
    path=destination(relative);path.parent.mkdir(parents=True,exist_ok=True)
    fd,temporary=tempfile.mkstemp(dir=path.parent,prefix='.'+path.name)
    try:
        with os.fdopen(fd,'wb') as stream:stream.write(data)
        os.chmod(temporary,0o600 if relative.as_posix()=='etc/easyduck/setup.env' else 0o644)
        os.replace(temporary,path)
    finally:
        if os.path.exists(temporary):os.unlink(temporary)
for record in records:
    if hashlib.sha256((target/record['path']).read_bytes()).hexdigest()!=record['sha256']:raise RuntimeError('Installed checksum mismatch')
print('Installed connection setup. Rollback manifest:',backup/'manifest.json')
if target==Path('/'):
    subprocess.run(['systemctl','daemon-reload'],check=True)
    subprocess.run(['systemctl','enable','--now','easyduck-setup.service','easyduck-gamepad-reconnect.timer','avahi-daemon.service','easyduck-network-access.service'],check=True)
    subprocess.run(['systemctl','restart','easyduck-setup.service'],check=True)
PY
