#!/usr/bin/env python3
"""Restore installation files and the release link from a backup manifest."""
import argparse
import hashlib
import json
import os
from pathlib import Path

from install import checked_path, replace_file


def restore(root, backup, allow_live=False):
    root = Path(root).resolve()
    backup = Path(backup).resolve()
    if root == Path('/') and not allow_live:
        raise ValueError('Live rollback requires --restore-on-board')
    entries = json.loads((backup / 'manifest.json').read_text(encoding='utf-8'))['files']
    # Validate all paths and backup contents before restoring any destination.
    for entry in entries:
        destination = checked_path(root, entry['path'], allow_symlink='symlink' in entry)
        if destination.exists() and destination.is_dir() and not destination.is_symlink():
            raise ValueError('Refusing to replace directory: ' + str(destination))
        if entry['existed'] and 'previous_symlink' not in entry:
            source = checked_path(backup / 'files', entry['path'])
            if not source.is_file():
                raise ValueError('Backup file missing: ' + str(source))
            expected = entry.get('backup_sha256')
            if expected and hashlib.sha256(source.read_bytes()).hexdigest() != expected:
                raise ValueError('Backup checksum mismatch: ' + str(source))

    for entry in reversed(entries):
        destination = checked_path(root, entry['path'], allow_symlink='symlink' in entry)
        if 'previous_symlink' in entry:
            temporary = destination.parent / ('.restore-current-' + backup.name)
            try:
                temporary.symlink_to(entry['previous_symlink'], target_is_directory=True)
                os.replace(temporary, destination)
            finally:
                if temporary.is_symlink():
                    temporary.unlink()
        elif entry['existed']:
            source = backup / 'files' / entry['path']
            replace_file(source, destination, source.stat().st_mode & 0o7777)
            os.utime(destination, ns=(source.stat().st_atime_ns, source.stat().st_mtime_ns))
        elif destination.is_file() or destination.is_symlink():
            destination.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--backup', type=Path, required=True)
    parser.add_argument('--restore-on-board', action='store_true')
    args = parser.parse_args()
    restore(args.root, args.backup, args.restore_on_board)


if __name__ == '__main__':
    main()
