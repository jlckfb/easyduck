#!/usr/bin/env python3
"""Check a staged image's Xbox support without loading modules or opening devices."""
import argparse
import json
from pathlib import Path
import subprocess

REQUIRED = ('CONFIG_INPUT_EVDEV=y', 'CONFIG_INPUT_JOYDEV=y',
            'CONFIG_INPUT_JOYSTICK=y', 'CONFIG_JOYSTICK_XPAD=m')
ALIAS = 'usb:v045Ep028Ed0110dcFFdscFFdpFFicFFisc5Dip01in00'


def check(root, release, kernel_config):
    root = Path(root).resolve()
    if not release or Path(release).name != release or release in ('.', '..'):
        raise ValueError('Invalid kernel release')
    config = set(Path(kernel_config).read_text().splitlines())
    missing = set(REQUIRED) - config
    if missing:
        raise ValueError('Missing kernel settings: ' + ', '.join(sorted(missing)))
    modules = root / 'lib/modules' / release
    candidates = [p for p in modules.rglob('xpad.ko*')
                  if p.name in ('xpad.ko', 'xpad.ko.xz', 'xpad.ko.zst', 'xpad.ko.gz')]
    if len(candidates) != 1:
        raise ValueError('Expected one packaged xpad module for ' + release)
    loader = root / 'etc/modules-load.d/easyduck-gamepad.conf'
    if 'xpad' not in [line.strip() for line in loader.read_text().splitlines()
                      if line.strip() and not line.lstrip().startswith('#')]:
        raise ValueError('Missing xpad boot loading rule')
    vermagic = subprocess.check_output(['modinfo', '-F', 'vermagic', str(candidates[0])], text=True).strip()
    if not vermagic or vermagic.split()[0] != release:
        raise ValueError('xpad vermagic does not match packaged kernel: ' + vermagic)
    # --resolve-alias only reads the staged module indexes; it never loads a module.
    resolved = subprocess.check_output(['modprobe', '--dirname', str(root), '--set-version', release,
                                        '--resolve-alias', ALIAS], text=True).splitlines()
    if 'xpad' not in resolved:
        raise ValueError('Packaged USB alias index does not resolve the receiver to xpad')
    return {'kernel_release': release, 'module': str(candidates[0].relative_to(root)),
            'vermagic': vermagic, 'boot_loader': str(loader.relative_to(root)),
            'receiver': '045e:028e', 'alias_resolves_to': resolved, 'hardware_test': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--kernel-release', required=True)
    parser.add_argument('--kernel-config', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(check(args.root, args.kernel_release, args.kernel_config), indent=2))
