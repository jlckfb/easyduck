"""Regression checks for incomplete or mismatched firmware inputs; no hardware access."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('check_gamepad', Path(__file__).parents[1] / 'check_gamepad.py')
gamepad = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gamepad)


class GamepadImageTests(unittest.TestCase):
    def fixture(self, root):
        config = root / 'config'
        config.write_text('\n'.join(gamepad.REQUIRED) + '\n')
        module = root / 'lib/modules/6.1.141/kernel/drivers/input/joystick/xpad.ko'
        module.parent.mkdir(parents=True)
        module.write_bytes(b'test module: modinfo is mocked')
        loader = root / 'etc/modules-load.d/easyduck-gamepad.conf'
        loader.parent.mkdir(parents=True)
        loader.write_text('# receiver\nxpad\n')
        return config, module, loader

    def test_complete_image_and_read_only_tools(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); config, _, _ = self.fixture(root)
            with patch.object(gamepad.subprocess, 'check_output', side_effect=['6.1.141 SMP aarch64\n', 'xpad\n']) as run:
                self.assertEqual(gamepad.check(root, '6.1.141', config)['receiver'], '045e:028e')
                self.assertIn('--resolve-alias', run.call_args_list[1].args[0])

    def test_config_alone_is_insufficient(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); config, module, _ = self.fixture(root); module.unlink()
            with self.assertRaisesRegex(ValueError, 'packaged xpad'):
                gamepad.check(root, '6.1.141', config)

    def test_missing_loader_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); config, _, loader = self.fixture(root); loader.write_text('# omitted\n')
            with self.assertRaisesRegex(ValueError, 'boot loading'):
                gamepad.check(root, '6.1.141', config)

    def test_wrong_kernel_module_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); config, _, _ = self.fixture(root)
            with patch.object(gamepad.subprocess, 'check_output', return_value='6.1.140 SMP aarch64\n'):
                with self.assertRaisesRegex(ValueError, 'vermagic'):
                    gamepad.check(root, '6.1.141', config)

    def test_missing_usb_alias_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); config, _, _ = self.fixture(root)
            with patch.object(gamepad.subprocess, 'check_output', side_effect=['6.1.141 SMP aarch64\n', '']):
                with self.assertRaisesRegex(ValueError, 'alias index'):
                    gamepad.check(root, '6.1.141', config)


if __name__ == '__main__':
    unittest.main()
