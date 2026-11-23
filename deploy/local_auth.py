"""System account authentication shared by setup and intercom."""
import ctypes
import ctypes.util
import hmac
from pathlib import Path
import threading
import time


def default_user():
    return 'lckfb'

class Auth:
    def __init__(self, username):
        self.username = username
        self.lock = threading.Lock()

    def verify(self, username, password):
        if username != self.username or not isinstance(password, str) or not 1 <= len(password) <= 1024:
            return False
        try:
            import pwd
            if pwd.getpwnam(username).pw_shell.endswith(('/nologin', '/false')):
                return False
            with Path('/etc/shadow').open() as stream:
                fields = next((line.rstrip().split(':') for line in stream if line.startswith(username + ':')), [])
            if len(fields) < 8 or not fields[1] or fields[1].startswith(('!', '*')):
                return False
            today = int(time.time() / 86400)
            if fields[7] and today >= int(fields[7]):
                return False
            if fields[2] == '0' or (fields[2] and fields[4] and int(fields[4]) >= 0 and today > int(fields[2]) + int(fields[4])):
                return False
            library = ctypes.CDLL(ctypes.util.find_library('crypt') or 'libcrypt.so.1')
            library.crypt.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
            library.crypt.restype = ctypes.c_char_p
            with self.lock:
                result = library.crypt(password.encode(), fields[1].encode())
                return bool(result) and hmac.compare_digest(result, fields[1].encode())
        except (OSError, ValueError, ImportError, KeyError):
            return False
