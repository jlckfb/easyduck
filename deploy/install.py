#!/usr/bin/env python3
"""Prepare or install application files, preserving configuration and rollback copies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINS = ('robotd', 'padd', 'robotctl', 'sounds')


def checked_path(root, relative, allow_symlink=False):
    root = Path(root).resolve()
    relative = Path(relative)
    if relative.is_absolute() or not relative.parts or '..' in relative.parts:
        raise ValueError('Invalid installation path: ' + str(relative))
    destination = root / relative
    try:
        destination.parent.resolve().relative_to(root)
    except ValueError as error:
        raise ValueError('Path leaves the target root: ' + str(relative)) from error
    if destination.is_symlink() and not allow_symlink:
        raise ValueError('Refusing to overwrite symlink: ' + str(destination))
    return destination


def mapping(profile, binaries):
    if profile not in ('board', 'examples'):
        raise ValueError('Unknown configuration profile: ' + str(profile))
    result = [(Path(binaries) / name, Path('opt/robot/daemon/bin') / name) for name in BINS]

    def tree(source, destination):
        for path in sorted(source.rglob('*')):
            parts = path.relative_to(source).parts
            if any(part in ('tests', '__pycache__', '.git') for part in parts):
                continue
            if path.name.startswith('.env') or path.suffix == '.pyc':
                continue
            if path.is_file():
                result.append((path, Path(destination) / path.relative_to(source)))

    models = json.loads((ROOT / 'models/manifest.json').read_text(encoding='utf-8'))
    for name, entry in sorted(models.items()):
        if Path(name).name != name or not name.endswith('.onnx'):
            raise ValueError('Invalid model filename: ' + name)
        source = ROOT / 'models' / name
        if hashlib.sha256(source.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('Model checksum mismatch: ' + name)
        result.append((source, Path('opt/robot/daemon/policies') / name))

    result.append((ROOT / 'web/server.py', Path('opt/robot/xgoduck-web/server.py')))
    for name in ('first_setup.py', 'connections.py', 'navigation.py'):
        result.append((ROOT / 'deploy/setup' / name, Path('opt/robot/setup') / name))
    tree(ROOT / 'deploy/setup/static', 'opt/robot/setup/static')
    tree(ROOT / 'web/assets', 'opt/robot/xgoduck-web/assets')
    for name in ('server.py', 'hardware.py', 'calibration_math.py', 'motion.py', 'servo_setup.py'):
        result.append((ROOT / 'calibration' / name, Path('opt/robot/calibration') / name))
    tree(ROOT / 'calibration/imu', 'opt/robot/calibration/imu')
    tree(ROOT / 'calibration/static', 'opt/robot/calibration/static')
    tree(ROOT / 'config' / profile, 'etc/robot')
    tree(ROOT / 'deploy/systemd', 'etc/systemd/system')
    tree(ROOT / 'deploy/sysusers.d', 'usr/lib/sysusers.d')
    tree(ROOT / 'deploy/alsa', 'var/lib/alsa')
    result.append((ROOT / 'LICENSE', Path('opt/robot/licenses/LICENSE')))
    result.append((ROOT / 'THIRD_PARTY_NOTICES.md', Path('opt/robot/licenses/THIRD_PARTY_NOTICES.md')))
    tree(ROOT / 'LICENSES', 'opt/robot/licenses')
    return result


def write_manifest(path, records):
    temporary = path.with_suffix('.json.new')
    temporary.write_text(json.dumps({'files': records}, indent=2) + '\n', encoding='utf-8')
    os.replace(temporary, path)


def replace_file(source, destination, mode):
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.' + destination.name + '-', dir=destination.parent)
    os.close(descriptor)
    try:
        shutil.copyfile(source, temporary)
        os.chmod(temporary, mode)
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def apply(target, profile, binaries, replace_config=False, allow_live=False):
    target = Path(target).resolve()
    if target == Path('/') and not allow_live:
        raise ValueError('Live installation requires --install-on-board; staging is the default.')
    entries = mapping(profile, Path(binaries))
    missing = [str(source) for source, _ in entries if not source.is_file()]
    if missing:
        raise ValueError('Missing inputs: ' + ', '.join(missing))

    selected = []
    for source, relative in entries:
        destination = checked_path(target, relative)
        if destination.exists() and not destination.is_file():
            raise ValueError('Destination is not a regular file: ' + str(destination))
        if relative.parts[:2] == ('etc', 'robot') and destination.exists() and not replace_config:
            continue
        selected.append((source, relative))

    link = checked_path(target, 'opt/robot/daemon/current', allow_symlink=True)
    if link.exists() and not link.is_symlink():
        raise ValueError('Expected current to be a symlink, not a directory or file: ' + str(link))
    backup = checked_path(target, Path('var/lib/easyduck/backups') / str(time.time_ns()))
    backup.mkdir(parents=True)
    records = []
    # Back up all overwritten files and persist the plan before changing targets.
    for source, relative in selected:
        destination = target / relative
        existed = destination.exists()
        record = {'path': relative.as_posix(), 'existed': existed,
                  'sha256': hashlib.sha256(source.read_bytes()).hexdigest()}
        if existed:
            saved = backup / 'files' / relative
            saved.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(destination, saved)
            record['backup_sha256'] = hashlib.sha256(saved.read_bytes()).hexdigest()
        records.append(record)
    if not link.is_symlink() or link.readlink() != Path('.'):
        record = {'path': 'opt/robot/daemon/current', 'existed': link.is_symlink(), 'symlink': '.'}
        if link.is_symlink():
            record['previous_symlink'] = str(link.readlink())
        records.append(record)
    write_manifest(backup / 'manifest.json', records)

    try:
        for source, relative in selected:
            mode = 0o755 if relative.parts[:4] == ('opt', 'robot', 'daemon', 'bin') else 0o644
            replace_file(source, target / relative, mode)
        if records and records[-1].get('symlink') == '.':
            temporary = link.parent / ('.current-' + backup.name)
            try:
                temporary.symlink_to('.', target_is_directory=True)
                os.replace(temporary, link)
            finally:
                if temporary.is_symlink():
                    temporary.unlink()
    except Exception as error:
        raise RuntimeError('Installation incomplete; rollback manifest: ' + str(backup / 'manifest.json')) from error
    return backup


def verify(target, manifest):
    target = Path(target).resolve()
    for entry in json.loads(Path(manifest).read_text(encoding='utf-8'))['files']:
        path = checked_path(target, entry['path'], allow_symlink='symlink' in entry)
        if 'symlink' in entry:
            if not path.is_symlink() or path.readlink() != Path(entry['symlink']):
                raise ValueError('Symlink mismatch: ' + str(path))
        elif not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('Installed checksum mismatch: ' + str(path))
    print('Installed file checksums OK.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT / 'build/stage')
    parser.add_argument('--profile', choices=['board', 'examples'], default='examples')
    parser.add_argument('--binaries', type=Path, default=ROOT / 'build/bin')
    parser.add_argument('--replace-config', action='store_true')
    parser.add_argument('--install-on-board', action='store_true')
    parser.add_argument('--verify', type=Path, help='Read-only check against an installation manifest')
    args = parser.parse_args()
    if args.verify:
        verify(args.root, args.verify)
    else:
        print(apply(args.root, args.profile, args.binaries, args.replace_config, args.install_on_board))


if __name__ == '__main__':
    main()
