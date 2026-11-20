"""On-demand MJPEG capture from a Linux USB Video Class device."""
from contextlib import ExitStack, contextmanager
import mmap
import os
from pathlib import Path
import select
import struct
import sys
import time


# Linux V4L2 single-plane ABI on little-endian 64-bit systems.
CAPTURE, MMAP = 1, 1
S_FMT, REQBUFS = 0xc0d05605, 0xc0145608
QUERYBUF, QBUF, DQBUF = 0xc0585609, 0xc058560f, 0xc0585611
STREAMON, STREAMOFF = 0x40045612, 0x40045613
JPEG = int.from_bytes(b'MJPG', 'little')


def camera_devices():
    for path in sorted(Path('/sys/class/video4linux').glob('video*')):
        driver = path / 'device/driver'
        if driver.exists() and driver.resolve().name == 'uvcvideo':
            yield Path('/dev') / path.name


@contextmanager
def capture_frames():
    if sys.platform != 'linux' or struct.calcsize('P') != 8 or sys.byteorder != 'little':
        raise OSError('Unsupported camera platform')
    import fcntl

    for device in camera_devices():
        with ExitStack() as resources:
            try:
                fd = os.open(device, os.O_RDWR | os.O_NONBLOCK)
                resources.callback(os.close, fd)

                def ioctl(command, data):
                    fcntl.ioctl(fd, command, data)

                fmt = bytearray(208)
                struct.pack_into('<I', fmt, 0, CAPTURE)
                struct.pack_into('<IIII', fmt, 8, 640, 480, JPEG, 0)
                ioctl(S_FMT, fmt)
                if struct.unpack_from('<I', fmt, 16)[0] != JPEG:
                    continue
                request = bytearray(20)
                struct.pack_into('<III', request, 0, 3, CAPTURE, MMAP)
                ioctl(REQBUFS, request)
                count = struct.unpack_from('<I', request)[0]
                if not 1 <= count <= 8:
                    continue
                buffers = []
                buffer = bytearray(88)
                for index in range(count):
                    struct.pack_into('<II', buffer, 0, index, CAPTURE)
                    struct.pack_into('<I', buffer, 60, MMAP)
                    ioctl(QUERYBUF, buffer)
                    offset = struct.unpack_from('<I', buffer, 64)[0]
                    length = struct.unpack_from('<I', buffer, 72)[0]
                    if not 0 < length <= 8 * 1024 * 1024:
                        raise OSError('Invalid camera buffer size')
                    mapped = mmap.mmap(fd, length, offset=offset)
                    resources.callback(mapped.close)
                    buffers.append(mapped)
                    ioctl(QBUF, buffer)
                capture_type = bytearray(struct.pack('<I', CAPTURE))
                ioctl(STREAMON, capture_type)
            except OSError:
                continue

            def frames():
                deadline = time.monotonic() + 2
                next_frame = 0.0
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0 or not select.select([fd], [], [], remaining)[0]:
                        raise TimeoutError('USB camera did not return a frame')
                    try:
                        ioctl(DQBUF, buffer)
                    except BlockingIOError:
                        continue
                    index, _, size, flags = struct.unpack_from('<IIII', buffer)
                    now = time.monotonic()
                    jpeg = None
                    if index >= len(buffers):
                        raise OSError('Invalid camera buffer index')
                    if not flags & 0x40 and 2 < size <= len(buffers[index]) and now >= next_frame:
                        jpeg = buffers[index][:size]
                    ioctl(QBUF, buffer)
                    if jpeg and jpeg.startswith(b'\xff\xd8'):
                        yield jpeg
                        next_frame = now + 1 / 15
                        deadline = time.monotonic() + 2

            try:
                yield frames()
            finally:
                try:
                    ioctl(STREAMOFF, capture_type)
                except OSError:
                    pass
            return
    raise OSError('No available MJPEG USB camera')
