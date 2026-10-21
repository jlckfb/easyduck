# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Marc Duclusaud
"""标定台使用的 LSM6DSV16X I2C 读取模块。

使用 Linux i2c-dev 访问陀螺仪、加速度计及 SFLP 四元数 FIFO，
后台线程提供姿态快照、有效性、数据年龄和错误状态。
输出兼容坐标由 _to_bno_frame 定义；标定台再使用保存的安装旋转。
初始化写入后回读检查，重试带退避；失去有效样本时不伪造新鲜数据。
寄存器页切换、软复位和重新配置有固定顺序，修改时需保留该顺序。
"""

from __future__ import annotations

import errno
import math
import os
import struct
import threading
import time
from dataclasses import dataclass

try:  # Linux I2C 需要 fcntl；其他平台仍可导入纯计算函数。
    import fcntl
except ImportError:  # pragma: no cover
    fcntl = None  # type: ignore[assignment]

I2C_SLAVE = 0x0703
I2C_SLAVE_FORCE = 0x0706

# ---- 寄存器地址（真值：lsm6dsv16x_reg.h）---------------------------------
REG_FUNC_CFG_ACCESS = 0x01
REG_FIFO_CTRL1 = 0x07
REG_FIFO_CTRL2 = 0x08
REG_FIFO_CTRL3 = 0x09
REG_FIFO_CTRL4 = 0x0A
REG_WHO_AM_I = 0x0F
REG_CTRL1_XL = 0x10
REG_CTRL2_G = 0x11
REG_CTRL3_C = 0x12
REG_CTRL6_C = 0x15
REG_CTRL8_XL = 0x17
REG_FIFO_STATUS1 = 0x1B
REG_FIFO_STATUS2 = 0x1C
REG_OUTX_L_G = 0x22
REG_OUTX_L_A = 0x28
REG_EMB_FUNC_EN_A = 0x04          # emb 页
REG_EMB_FUNC_FIFO_EN_A = 0x44     # emb 页
REG_SFLP_ODR = 0x5E               # emb 页
REG_EMB_FUNC_INIT_A = 0x66        # emb 页
REG_FIFO_DATA_OUT_TAG = 0x78
FIFO_FRAME_LEN = 7                # tag(1) + 3×u16 小端(6)

WHO_AM_I_EXPECTED = 0x70
WHO_AM_I_ADDRS = (0x6A, 0x6B)

BIT_SW_POR = 0x04                 # FUNC_CFG_ACCESS bit2
BIT_EMB_FUNC_PAGE = 0x80          # FUNC_CFG_ACCESS bit7

CTRL3_BDU_IF_INC = 0x44           # BDU=bit6, IF_INC=bit2
CTRL3_SW_RESET = 0x01             # CTRL3_C bit0，自清零；软复位后必须重跑整条初始化
CTRL6_GYRO_FS_500DPS = 0x02       # FS_G[3:0] = ±500 dps
CTRL8_XL_FS_4G = 0x01             # FS_XL[1:0] = ±4 g
ODR_240HZ = 0x07                  # ODR[3:0]，bit[6:4]=000 高性能
FIFO_MODE_STREAM = 0x06           # FIFO_CTRL4 fifo_mode[2:0]
EMB_SFLP_GAME_EN = 0x02           # EMB_FUNC_EN_A bit1
EMB_SFLP_GAME_FIFO_EN = 0x02      # EMB_FUNC_FIFO_EN_A bit1（同时清掉 0x30 = 其余 SFLP）
EMB_SFLP_GAME_INIT = 0x02         # EMB_FUNC_INIT_A bit1（缺这条融合块永不出数据）
SFLP_ODR_SHIFT = 3                # sflp_game_odr = bits[5:3]
SFLP_ODR_MASK = 0x38
SFLP_ODR_CODE_60HZ = 0x02
SFLP_ODR_CODE_30HZ = 0x01
SFLP_ODR_CODE_120HZ = 0x03
TAG_SFLP_GAME_ROTATION = 0x13     # tag = byte >> 3

GYRO_RAD_PER_LSB = 0.01750 * math.pi / 180.0   # ±500dps: 17.50 mdps/LSB
ACC_G_PER_LSB = 0.000122                       # ±4g: 0.122 mg/LSB
QUAT_NORM_TOL = 0.05              # |q| 偏离 1 超过 ±0.05 → 不可信
MAX_DRAIN_FRAMES = 16             # 每 tick 最多排水帧数（防 FIFO 溢出时长时间占总线）
MAX_RAW_GYRO_RAD_S = 40.0         # 量程±500dps=8.7rad/s；>40 只可能是垃圾数据
MAX_RAW_ACC_G = 8.0               # 量程±4g；>8 只可能是垃圾数据

I2C_SLAVE_ERRNOS = frozenset(
    code
    for code in (
        errno.EIO,
        errno.ENXIO,
        getattr(errno, "EREMOTEIO", None),
        getattr(errno, "ETIMEDOUT", None),
    )
    if code is not None
) | {errno.EAGAIN, getattr(errno, "EWOULDBLOCK", errno.EAGAIN)}

# 句柄级致命错误：同一个 fd 上重试永远救不回来，只能关掉重开。
#   EBADF  = 别处 close 了 fd；ENOENT = 设备节点被 unbound（节点已不在）
_FATAL_HANDLE_ERRNOS = frozenset(
    code for code in (errno.EBADF, errno.ENOENT) if code is not None
)


# ---------------------------------------------------------------------------
# 四元数工具（与 imu_reader 中同名函数语义一致；本模块自带避免循环依赖）
# ---------------------------------------------------------------------------
def _normalize_quat(q: tuple[float, float, float, float]) -> tuple[float, float, float, float]:
    w, x, y, z = q
    norm = math.sqrt(w * w + x * x + y * y + z * z)
    if not math.isfinite(norm) or norm <= 1e-12:
        return (1.0, 0.0, 0.0, 0.0)
    return (w / norm, x / norm, y / norm, z / norm)


def _quat_mul(
    a: tuple[float, float, float, float],
    b: tuple[float, float, float, float],
) -> tuple[float, float, float, float]:
    w1, x1, y1, z1 = a
    w2, x2, y2, z2 = b
    return (
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
    )


def _quat_apply_inverse(
    quat: tuple[float, float, float, float],
    vec: tuple[float, float, float],
) -> tuple[float, float, float]:
    """等价于 ``imu_reader.quat_apply_inverse`` 的纯标准库实现（不依赖 numpy）。"""
    w, x, y, z = quat
    vx, vy, vz = vec
    tx = 2.0 * (y * vz - z * vy)
    ty = 2.0 * (z * vx - x * vz)
    tz = 2.0 * (x * vy - y * vx)
    return (
        vx - w * tx + (y * tz - z * ty),
        vy - w * ty + (z * tx - x * tz),
        vz - w * tz + (x * ty - y * tx),
    )


@dataclass(frozen=True)
class LsmImuSnapshot:
    """与 ``imu_reader.IMUSnapshot`` 同字段的本地快照（下游只按属性取值，鸭子类型足够）。

    刻意**不** import imu_reader.IMUSnapshot：那会构成循环依赖。
    """

    timestamp_s: float
    quat: tuple[float, float, float, float]
    gyro: tuple[float, float, float]
    acc: tuple[float, float, float]
    valid: bool
    error_count: int


def _is_i2c_error(exc: BaseException) -> bool:
    return isinstance(exc, OSError) and exc.errno in I2C_SLAVE_ERRNOS


# ---------------------------------------------------------------------------
# I2C 裸访问
# ---------------------------------------------------------------------------
class _I2cDev:
    """i2c-dev 裸访问：open + ioctl(I2C_SLAVE) + write/read。

    所有读写都带 short-write 处理、Errno 5/6/121 退避重试；重试次数有界且每次
    至少 sleep 2ms —— 绝不空转猛读（见文件头约束 2）。
    """

    def __init__(self, bus: int, addr: int, retries: int = 3, retry_delay_s: float = 0.002) -> None:
        self.bus = bus
        self.addr = addr
        self.retries = max(1, int(retries))
        self.retry_delay_s = float(retry_delay_s)
        self.fd = os.open(f"/dev/i2c-{bus}", os.O_RDWR)
        try:
            fcntl.ioctl(self.fd, I2C_SLAVE, addr)
        except OSError:
            fcntl.ioctl(self.fd, I2C_SLAVE_FORCE, addr)

    def close(self) -> None:
        try:
            os.close(self.fd)
        except OSError:
            pass

    def _retry(self, fn):
        attempt = 0
        while True:
            try:
                return fn()
            except OSError as exc:
                attempt += 1
                if attempt >= self.retries or not _is_i2c_error(exc):
                    raise
                time.sleep(self.retry_delay_s * attempt)

    def _write_all(self, payload: bytes) -> None:
        view = memoryview(payload)
        while view:
            written = os.write(self.fd, view)
            if written <= 0:
                raise OSError(errno.EIO, "i2c short write")
            view = view[written:]

    def _read_exact(self, n: int) -> bytes:
        buf = bytearray()
        while len(buf) < n:
            chunk = os.read(self.fd, n - len(buf))
            if not chunk:
                raise OSError(errno.EIO, "i2c short read")
            buf.extend(chunk)
        return bytes(buf)

    def read_reg(self, reg: int) -> int:
        return self._retry(lambda: self._read_reg_once(reg))

    def _read_reg_once(self, reg: int) -> int:
        self._write_all(bytes([reg & 0xFF]))
        return self._read_exact(1)[0]

    def read_n(self, reg: int, n: int) -> bytes:
        return self._retry(lambda: self._read_n_once(reg, n))

    def _read_n_once(self, reg: int, n: int) -> bytes:
        self._write_all(bytes([reg & 0xFF]))
        return self._read_exact(n)

    def write_reg(self, reg: int, value: int) -> None:
        self._retry(lambda: self._write_all(bytes([reg & 0xFF, value & 0xFF])))


class Lsm6dsv16xError(RuntimeError):
    """LSM6DSV16X 探测/初始化失败（不匹配、回读不符等）。"""


# ---------------------------------------------------------------------------
# 线程化读取器
# ---------------------------------------------------------------------------
class Lsm6dsv16xReader:
    """LSM6DSV16X（I2C1，0x6A/0x6B）后台读取线程，输出“BNO 贴装等效系”的 IMU 数据。

    与 BNO 后端语义对齐：

    * ``valid`` = 收到传感器数据 **且** 姿态年龄 ≤ ``IMU_QUAT_MAX_AGE_S``；
    * 失联 0.6s → ``valid=False``（保留旧值，不伪造新鲜度）；
      1.2s → 退避重初始化（5→10→20→40→60s）；2.0s → ``failed=True``；
    * ``get_latest()`` / ``get_status()`` 永不抛异常。
    """

    def __init__(
        self,
        i2c_bus: int,
        frequency_hz: float = 50.0,
        warn_interval_s: float = 1.0,
        addr_candidates: tuple[int, ...] = WHO_AM_I_ADDRS,
        sflp_odr_hz: float = 60.0,
        delta_quat: tuple[float, float, float, float] = (1.0, 0.0, 0.0, 0.0),
    ) -> None:
        if frequency_hz <= 0:
            raise ValueError("frequency_hz must be > 0")

        # 与 imu_reader 内同名常量保持一致（那边是 BNO 后端实测调出来的值）。
        self._stall_valid_s = 0.6
        self._stall_recover_s = 1.2
        self._stall_fail_s = 2.0
        self._quat_max_age_s = 1.0
        self._max_poll_hz = 200.0

        self._bus = int(i2c_bus)
        self._addr: int | None = None
        self._addr_candidates = tuple(int(a) for a in addr_candidates) or WHO_AM_I_ADDRS
        self._period_s = 1.0 / float(frequency_hz)
        self._warn_interval_s = float(warn_interval_s)
        self._min_read_interval_s = 1.0 / self._max_poll_hz
        self._delta_quat = _normalize_quat(tuple(float(v) for v in delta_quat))  # type: ignore[arg-type]
        self._sflp_odr_code = self._odr_code(float(sflp_odr_hz))

        self._dev: _I2cDev | None = None
        self._lock = threading.Lock()
        self._stop_event = threading.Event()
        self._thread = threading.Thread(target=self._run_loop, name="imu-reader-lsm", daemon=True)

        now = time.perf_counter()
        self._snapshot = LsmImuSnapshot(
            timestamp_s=now,
            quat=(1.0, 0.0, 0.0, 0.0),
            gyro=(0.0, 0.0, 0.0),
            acc=(0.0, 0.0, 0.0),
            valid=False,
            error_count=0,
        )
        self._error_count = 0
        self._last_warn_s = 0.0
        self._imu_failed = False
        self._closed = False
        self._stall_count = 0
        self._stall_reason = ""
        self._last_report_s = now
        self._last_quat_s = now
        self._last_recover_s = 0.0
        self._recover_backoff_s = 5.0
        self._quat_missing_warned = False

    # ---- 公开接口（与 BNO 后端同名同签名）--------------------------------
    def start(self) -> None:
        if not self._thread.is_alive():
            self._init_device()
            self._thread.start()

    def stop(self, timeout_s: float = 1.0) -> None:
        self._stop_event.set()
        self._closed = True          # 停掉后不再做任何恢复/重初始化尝试
        if self._thread.is_alive():
            self._thread.join(timeout=timeout_s)
        dev = self._dev
        self._dev = None
        if dev is not None:
            dev.close()

    @property
    def failed(self) -> bool:
        """True = 连续 ``IMU_STALL_FAIL_S`` 秒没有可用姿态数据（下游必须禁止行走）。"""
        return self._imu_failed

    def get_latest(self) -> LsmImuSnapshot:
        try:
            with self._lock:
                return self._snapshot
        except Exception:  # noqa: BLE001 - 契约：永不抛异常
            now = time.perf_counter()
            return LsmImuSnapshot(
                timestamp_s=now,
                quat=(1.0, 0.0, 0.0, 0.0),
                gyro=(0.0, 0.0, 0.0),
                acc=(0.0, 0.0, 0.0),
                valid=False,
                error_count=self._error_count,
            )

    def get_status(self) -> dict[str, float | int | bool | str | None]:
        """13 键超集（与 BNO 后端同键）+ ``backend``。永不抛异常。"""
        try:
            snap = self.get_latest()
            now = time.perf_counter()
            return {
                "valid": bool(snap.valid),
                "age_s": max(0.0, now - snap.timestamp_s),
                "error_count": int(snap.error_count),
                "target_frequency_hz": 1.0 / self._period_s,
                "bus": self._bus,
                "addr": self._addr if self._addr is not None else -1,
                "failed": bool(self._imu_failed),
                "stall_count": int(self._stall_count),
                "report_age_s": max(0.0, now - self._last_report_s),
                "quat_age_s": max(0.0, now - self._last_quat_s),
                "gpio_backend": None,
                "int_pin": None,
                "rst_pin": None,
                "backend": "lsm6dsv16x",
                "stall_reason": self._stall_reason,
                "delta_quat": tuple(self._delta_quat),
                "quat_norm": self._quat_norm(snap.quat),
            }
        except Exception:  # noqa: BLE001 - 契约：永不抛异常
            return {
                "valid": False,
                "age_s": float("inf"),
                "error_count": self._error_count,
                "target_frequency_hz": 1.0 / self._period_s,
                "bus": self._bus,
                "addr": self._addr if self._addr is not None else -1,
                "failed": True,
                "stall_count": self._stall_count,
                "report_age_s": float("inf"),
                "quat_age_s": float("inf"),
                "gpio_backend": None,
                "int_pin": None,
                "rst_pin": None,
                "backend": "lsm6dsv16x",
                "stall_reason": "status_error",
                "delta_quat": tuple(self._delta_quat),
                "quat_norm": 0.0,
            }

    # ---- 内部工具 --------------------------------------------------------
    @staticmethod
    def _odr_code(odr_hz: float) -> int:
        """SFLP game ODR 档位（bits[5:3] 里的 code）：15/30/60/120/240 → 0..4。"""
        table = ((15.0, 0), (30.0, 1), (60.0, 2), (120.0, 3), (240.0, 4))
        for hz, code in table:
            if abs(odr_hz - hz) < 1e-6:
                return code
        # 非标准档 → 取不超过请求值的最大档（至少 15Hz）
        code = 0
        for hz, value in table:
            if odr_hz >= hz:
                code = value
        return code

    @staticmethod
    def _quat_norm(q: tuple[float, float, float, float]) -> float:
        return math.sqrt(sum(float(v) * float(v) for v in q))

    def _quat_is_sane(self, q: tuple[float, float, float, float]) -> bool:
        if not all(math.isfinite(float(v)) for v in q):
            return False
        norm = self._quat_norm(q)
        if norm <= 1e-6:            # 全零 → 诚实判无效
            return False
        return abs(norm - 1.0) <= QUAT_NORM_TOL

    def _raw_is_sane(
        self,
        gyro: tuple[float, float, float],
        acc: tuple[float, float, float],
    ) -> bool:
        for value in gyro:
            if not math.isfinite(value) or abs(value) > MAX_RAW_GYRO_RAD_S:
                return False
        for value in acc:
            if not math.isfinite(value) or abs(value) > MAX_RAW_ACC_G:
                return False
        return True

    def _warn(self, message: str) -> None:
        now = time.perf_counter()
        if (now - self._last_warn_s) >= self._warn_interval_s:
            print(message, end="\r\n", flush=True)
            self._last_warn_s = now

    def _require_dev(self) -> _I2cDev:
        dev = self._dev
        if dev is None:
            raise Lsm6dsv16xError("LSM6DSV16X 设备未打开")
        return dev

    # ---- 寄存器读写 + 回读比对 -------------------------------------------
    def _write_verify(self, reg: int, value: int, name: str) -> int:
        """写寄存器并回读比对；不符立即抛错。"""
        dev = self._require_dev()
        dev.write_reg(reg, value)
        readback = dev.read_reg(reg)
        if readback != (value & 0xFF):
            raise Lsm6dsv16xError(
                f"{name}(0x{reg:02X}) 回读不符: 写入 0x{value:02X} 实读 0x{readback:02X}"
            )
        return readback

    def _read_modify_write(self, reg: int, clear_mask: int, set_bits: int, name: str) -> int:
        """读-改-写：保留未涉及位（真值表要求），再回读比对。"""
        dev = self._require_dev()
        old = dev.read_reg(reg)
        new = (old & ~clear_mask & 0xFF) | (set_bits & 0xFF)
        return self._write_verify(reg, new, name)

    def _in_emb_page(self) -> bool:
        return bool(self._require_dev().read_reg(REG_FUNC_CFG_ACCESS) & BIT_EMB_FUNC_PAGE)

    def _emb_page_enter(self) -> None:
        dev = self._require_dev()
        old = dev.read_reg(REG_FUNC_CFG_ACCESS)
        dev.write_reg(REG_FUNC_CFG_ACCESS, old | BIT_EMB_FUNC_PAGE)
        if not (dev.read_reg(REG_FUNC_CFG_ACCESS) & BIT_EMB_FUNC_PAGE):
            raise Lsm6dsv16xError("FUNC_CFG_ACCESS 未能切到 embedded 功能页")

    def _emb_page_exit(self) -> None:
        dev = self._require_dev()
        old = dev.read_reg(REG_FUNC_CFG_ACCESS)
        dev.write_reg(REG_FUNC_CFG_ACCESS, old & ~BIT_EMB_FUNC_PAGE & 0xFF)
        if dev.read_reg(REG_FUNC_CFG_ACCESS) & BIT_EMB_FUNC_PAGE:
            raise Lsm6dsv16xError("FUNC_CFG_ACCESS 未能退回主功能页")

    # ---- 探活 / 初始化 ---------------------------------------------------
    def _probe_device(self, dev: _I2cDev) -> bool:
        """只读探活：WHO_AM_I == 0x70。任何 I2C 错误都按“不在线”处理。"""
        try:
            return dev.read_reg(REG_WHO_AM_I) == WHO_AM_I_EXPECTED
        except OSError:
            return False

    def _soft_por(self, dev: _I2cDev) -> None:
        """软 POR：FUNC_CFG_ACCESS |= 0x04，然后 20×20ms 轮询 WHO_AM_I 恢复。

        只在 WHO_AM_I 读错时调用，且必须在任何 CTRL 写入**之前**（见文件头约束 4）。
        复位自清零并会一并清掉 bank 位（照 imu_i2c.rs / reg.h 的语义），因此这里不
        额外折腾 FUNC_CFG_ACCESS —— 万一没清干净，_configure 的 _emb_page_enter 会纠正。
        """
        dev.write_reg(REG_FUNC_CFG_ACCESS, BIT_SW_POR)
        for _ in range(20):
            time.sleep(0.02)
            if self._probe_device(dev):
                return
        raise Lsm6dsv16xError("软 POR 后 WHO_AM_I 仍未恢复 0x70")

    def _hard_reset(self, dev: _I2cDev) -> None:
        """CTRL3_C.sw_reset（bit0，自清零）——LSM 的僵死恢复**不需要 GPIO RST**。

        软复位会把所有 CTRL 清回默认值，所以调用方必须紧接着重跑整条初始化序列。
        轮询带 2ms sleep（有界、有节流），绝不空转猛读。
        """
        dev.write_reg(REG_CTRL3_C, CTRL3_SW_RESET)
        for _ in range(50):
            time.sleep(0.002)
            try:
                if not (dev.read_reg(REG_CTRL3_C) & CTRL3_SW_RESET):
                    return
            except OSError:
                return  # 复位瞬间总线可能短暂无应答
        raise Lsm6dsv16xError("CTRL3_C.sw_reset 未自清零，芯片异常")

    def _init_device(self) -> None:
        """打开设备 + 六步初始化。失败置 failed=True，绝不抛异常给调用方。"""
        try:
            dev = self._open_device()
        except Exception as exc:  # noqa: BLE001 - start() 不得抛异常
            self._dev = None
            self._imu_failed = True
            self._stall_reason = f"open_failed: {exc}"
            print(f"Warning: LSM6DSV16X 初始化失败: {exc}", end="\r\n", flush=True)
            return
        self._dev = dev
        try:
            self._configure(dev)
        except Exception as exc:  # noqa: BLE001 - start() 不得抛异常
            self._imu_failed = True
            self._stall_reason = f"configure_failed: {exc}"
            print(f"Warning: LSM6DSV16X 初始化失败: {exc}", end="\r\n", flush=True)
            return
        self._imu_failed = False
        self._stall_reason = ""
        print(
            f"IMU: LSM6DSV16X 就绪 bus={self._bus} addr=0x{self._addr:02X} "
            f"SFLP={1 << self._sflp_odr_code}~60Hz 量程±500dps/±4g",
            end="\r\n",
            flush=True,
        )

    def _open_device(self) -> _I2cDev:
        """探活 0x6A / 0x6B。只有 WHO_AM_I 读错的那条路径才允许软 POR。

        ★每个候选地址的成败状态必须是**本候选自己的**：0x6A 不在线留下的 last_error
        绝不能让 finally 把 0x6B 那个刚打开、正要返回的 fd 关掉（板上实测过一次，
        现象是 configure 第一步就 EBADF）。这里用显式关闭，不靠 finally 兜底。
        """
        last_error: Exception | None = None
        for addr in self._addr_candidates:
            try:
                dev = _I2cDev(self._bus, addr)
            except OSError as exc:
                last_error = exc
                continue

            if self._probe_device(dev):
                self._addr = addr
                return dev

            # WHO_AM_I 在线但读错 → 软 POR 抢救一次（仍在任何 CTRL 写入之前）
            self._dev = dev
            try:
                self._soft_por(dev)
            except (OSError, Lsm6dsv16xError) as exc:
                last_error = Lsm6dsv16xError(f"0x{addr:02X} 软 POR 未救回: {exc}")
                self._dev = None
                dev.close()
            else:
                self._dev = None
                self._addr = addr
                return dev

        raise Lsm6dsv16xError(
            f"LSM6DSV16X 未在 /dev/i2c-{self._bus} 的 "
            f"{'/'.join(f'0x{a:02X}' for a in self._addr_candidates)} 应答"
            + (f": {last_error}" if last_error is not None else "")
        )

    def _configure(self, dev: _I2cDev) -> None:
        """按固定顺序初始化，每条配置写入后回读比对。"""
        # 1. WHO_AM_I 必须 0x70；POR 已在使用前处理，这里只复核
        who = dev.read_reg(REG_WHO_AM_I)
        if who != WHO_AM_I_EXPECTED:
            raise Lsm6dsv16xError(
                f"WHO_AM_I=0x{who:02X}，期望 0x{WHO_AM_I_EXPECTED:02X}"
            )

        # 2. 主功能页 CTRL：BDU+IF_INC / 陀螺量程 / 加速度量程 / 两个 ODR
        self._write_verify(REG_CTRL3_C, CTRL3_BDU_IF_INC, "CTRL3_C")
        self._write_verify(REG_CTRL6_C, CTRL6_GYRO_FS_500DPS, "CTRL6_C")
        self._write_verify(REG_CTRL8_XL, CTRL8_XL_FS_4G, "CTRL8_XL")
        self._write_verify(REG_CTRL1_XL, ODR_240HZ, "CTRL1_XL")
        self._write_verify(REG_CTRL2_G, ODR_240HZ, "CTRL2_G")

        # 3. FIFO 水位线/批量全关（BDR 字会挤掉四元数）
        self._write_verify(REG_FIFO_CTRL1, 0x00, "FIFO_CTRL1")
        self._write_verify(REG_FIFO_CTRL2, 0x00, "FIFO_CTRL2")
        self._write_verify(REG_FIFO_CTRL3, 0x00, "FIFO_CTRL3")

        # 4. emb 页：把 SFLP game rotation 挂到 FIFO（同时清掉 0x30 其余 SFLP 源）
        self._emb_page_enter()
        try:
            self._read_modify_write(
                REG_EMB_FUNC_FIFO_EN_A, 0x3E, EMB_SFLP_GAME_FIFO_EN, "EMB_FUNC_FIFO_EN_A"
            )
        finally:
            self._emb_page_exit()

        # 5. FIFO 流模式（满时新覆盖旧，读晚也拿到当前姿态）
        self._write_verify(REG_FIFO_CTRL4, FIFO_MODE_STREAM, "FIFO_CTRL4")

        # 6. emb 页：SFLP ODR=60Hz → 使能 SFLP game → 初始化融合块
        self._emb_page_enter()
        try:
            self._read_modify_write(
                REG_SFLP_ODR,
                SFLP_ODR_MASK,
                self._sflp_odr_code << SFLP_ODR_SHIFT,
                "SFLP_ODR",
            )
            self._read_modify_write(REG_EMB_FUNC_EN_A, 0x00, EMB_SFLP_GAME_EN, "EMB_FUNC_EN_A")
            self._read_modify_write(
                REG_EMB_FUNC_INIT_A, 0x00, EMB_SFLP_GAME_INIT, "EMB_FUNC_INIT_A"
            )
        finally:
            self._emb_page_exit()

        # 7. 排一遍配置期间积在 FIFO 里的旧字（有界 + 带 sleep，不算猛读）。
        #    不做的话流模式下会先端出最多 256 个字（约 1s 陈旧的姿态），
        #    valid 会是真但数据是老的。
        self._flush_fifo()

    def _flush_fifo(self, max_rounds: int = 64) -> int:
        """有界排空 FIFO，返回排掉的字数。只在初始化/重初始化后调用一次。"""
        dev = self._require_dev()
        dropped = 0
        for _ in range(max_rounds):
            status = dev.read_n(REG_FIFO_STATUS1, 2)
            pending = status[0] | ((status[1] & 0x01) << 8)
            if pending <= 0:
                break
            for _ in range(min(pending, MAX_DRAIN_FRAMES)):
                dev.read_n(REG_FIFO_DATA_OUT_TAG, FIFO_FRAME_LEN)
                dropped += 1
            time.sleep(0.001)
        return dropped

    # ---- 每 tick 读取路径 ------------------------------------------------
    def _read_fifo_quat(self) -> tuple[float, float, float, float] | None:
        """排水 FIFO，返回最后一条 tag==0x13 的四元数（无则 None）。"""
        dev = self._require_dev()
        status = dev.read_n(REG_FIFO_STATUS1, 2)
        pending = status[0] | ((status[1] & 0x01) << 8)
        if pending <= 0:
            return None
        quat: tuple[float, float, float, float] | None = None
        for _ in range(min(pending, MAX_DRAIN_FRAMES)):
            word = dev.read_n(REG_FIFO_DATA_OUT_TAG, FIFO_FRAME_LEN)
            if len(word) < FIFO_FRAME_LEN:
                break
            tag = word[0] >> 3
            if tag != TAG_SFLP_GAME_ROTATION:
                continue
            x, y, z = struct.unpack("<3e", word[1:7])
            sumsq = x * x + y * y + z * z
            if sumsq > 1.0:
                sumsq = 1.0
            w = math.sqrt(max(0.0, 1.0 - sumsq))
            quat = (w, x, y, z)
        return quat

    def _read_raw(self) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
        dev = self._require_dev()
        gx, gy, gz = struct.unpack("<3h", dev.read_n(REG_OUTX_L_G, 6))
        ax, ay, az = struct.unpack("<3h", dev.read_n(REG_OUTX_L_A, 6))
        gyro = (gx * GYRO_RAD_PER_LSB, gy * GYRO_RAD_PER_LSB, gz * GYRO_RAD_PER_LSB)
        acc = (ax * ACC_G_PER_LSB, ay * ACC_G_PER_LSB, az * ACC_G_PER_LSB)
        return gyro, acc

    def _read_once(
        self,
    ) -> tuple[tuple[float, float, float, float] | None, tuple[float, float, float], tuple[float, float, float]]:
        quat = self._read_fifo_quat()
        gyro, acc = self._read_raw()
        return quat, gyro, acc

    def _to_bno_frame(
        self,
        quat: tuple[float, float, float, float],
        gyro: tuple[float, float, float],
        acc: tuple[float, float, float],
    ) -> tuple[tuple[float, float, float, float], tuple[float, float, float], tuple[float, float, float]]:
        """LSM 传感器系 → BNO 贴装等效系（读取层内部完成，下游零改动）。

        ``q_out = norm(q_lsm ⊗ DELTA)``、``gyro/acc_out = quat_apply_inverse(DELTA, ·)``。
        """
        delta = self._delta_quat
        if delta == (1.0, 0.0, 0.0, 0.0):
            return quat, gyro, acc
        return (
            _normalize_quat(_quat_mul(quat, delta)),
            _quat_apply_inverse(delta, gyro),
            _quat_apply_inverse(delta, acc),
        )

    # ---- 读循环 ----------------------------------------------------------
    def _run_loop(self) -> None:
        latest_q = (1.0, 0.0, 0.0, 0.0)
        latest_g = (0.0, 0.0, 0.0)
        latest_a = (0.0, 0.0, 0.0)
        now = time.perf_counter()
        self._last_report_s = now
        self._last_quat_s = now  # 让 quat_age 从循环真正开始算，而不是从 0 算
        self._last_read_s = 0.0
        next_tick = time.perf_counter()
        if self._dev is None:
            # start() 里初始化就失败了：这里补一次；仍失败则交给 _check_stall 的
            # 退避恢复去救（绝不空转猛读，见文件头约束 2）
            self._init_device()

        while not self._stop_event.is_set():
            # 限速 ≤ IMU_MAX_POLL_HZ：寄存器型设备不会被读僵死，但保持与 BNO 后端同样的硬限速
            now = time.perf_counter()
            if (now - self._last_read_s) < self._min_read_interval_s:
                time.sleep(0.001)
                self._check_stall(now)
                continue
            self._last_read_s = now

            got_quat = False
            try:
                quat, gyro, acc = self._read_once()
                if quat is not None and self._quat_is_sane(quat) and self._raw_is_sane(gyro, acc):
                    q_out, g_out, a_out = self._to_bno_frame(quat, gyro, acc)
                    # 旋完再复核一次模长：DELTA 本身是单位四元数，这里只做兜底
                    if self._quat_is_sane(q_out):
                        latest_q = q_out
                        latest_g = g_out
                        latest_a = a_out
                        got_quat = True
                        self._last_quat_s = now
                if got_quat:
                    self._snapshot = LsmImuSnapshot(
                        timestamp_s=now,
                        quat=latest_q,
                        gyro=latest_g,
                        acc=latest_a,
                        valid=True,
                        error_count=self._error_count,
                    )
                    self._last_report_s = now
                    if self._stall_count:
                        print(
                            f"IMU: 数据已恢复（此前失联检查 {self._stall_count} 次）",
                            end="\r\n",
                            flush=True,
                        )
                    self._stall_count = 0
                    self._imu_failed = False
                    self._stall_reason = ""
                    self._quat_missing_warned = False
                    self._recover_backoff_s = 5.0
                else:
                    if not self._quat_missing_warned:
                        self._quat_missing_warned = True
                        self._warn(
                            "Warning: IMU 有传感器读数但没有可用的 SFLP 姿态四元数"
                            "（或模长越界/全零）—— 策略没有重力方向，行走必须被禁止"
                        )
                    self._check_stall(now)
            except Exception as exc:  # noqa: BLE001 - 读循环不许退出
                self._error_count += 1
                self._warn(f"Warning: LSM6DSV16X read failed ({self._error_count}): {exc}")
                self._check_stall(now)
                time.sleep(0.02)

            # 目标频率节流（frequency_hz=200 时 SFLP 只有 60Hz，拿不到新帧就沿用上一帧）
            next_tick += self._period_s
            sleep_s = next_tick - time.perf_counter()
            if sleep_s > 0:
                time.sleep(sleep_s)
            else:
                next_tick = time.perf_counter()
                if (now - self._last_read_s) >= self._min_read_interval_s:
                    time.sleep(0.001)

    def _check_stall(self, now: float) -> None:
        """失联分级：valid=False → 退避重初始化 → failed=True（对齐 BNO 后端语义）。"""
        idle = now - self._last_report_s
        if idle <= self._stall_valid_s:
            return
        with self._lock:
            prev = self._snapshot
            if prev.valid:
                self._snapshot = LsmImuSnapshot(
                    timestamp_s=prev.timestamp_s,   # 保留旧值，绝不伪造新鲜度
                    quat=prev.quat,
                    gyro=prev.gyro,
                    acc=prev.acc,
                    valid=False,
                    error_count=self._error_count,
                )
        if self._closed:
            return
        if idle > self._stall_recover_s and (now - self._last_recover_s) > self._recover_backoff_s:
            self._last_recover_s = now
            self._stall_count += 1
            # 退避：5s -> 10s -> 20s -> 40s -> 60s
            self._recover_backoff_s = min(self._recover_backoff_s * 2.0, 60.0)
            self._reinitialize()
        # ★failed 放在恢复尝试**之后**：本轮已经重新拉起来就不该再报 failed
        if idle > self._stall_fail_s and not self._imu_failed:
            self._imu_failed = True
            self._stall_reason = f"no_usable_attitude_{idle:.1f}s"
            print(
                "Warning: LSM6DSV16X 连续无可用姿态数据 —— 行走必须被禁止",
                end="\r\n",
                flush=True,
            )

    def _reinitialize(self) -> None:
        """就地重初始化；句柄坏了就重开；最后升级为 CTRL3_C.sw_reset + 重跑初始化。

        全程不碰 GPIO27/22（BNO 占用）。软复位后 CTRL 全回默认值，所以复位成功
        必须紧接着 _configure，否则融合块没有输入流。

        ★句柄级错误（EBADF / ENOENT：别处 close 了 fd、或设备节点被 unbound）
        在同一个 fd 上重试是救不回来的，必须关掉旧句柄重新 _open_device() ——
        否则 failed 永久为真、机器人永久拒走且不自愈。
        """
        dev = self._dev
        if dev is None:
            if self._closed:
                return
            self._init_device()
            return
        try:
            self._configure(dev)
        except OSError as exc:
            if getattr(exc, "errno", None) not in _FATAL_HANDLE_ERRNOS:
                self._warn(f"Warning: LSM6DSV16X 重初始化 I2C 错误: {exc}")
                return
            self._warn(f"Warning: LSM6DSV16X 句柄失效({exc})，重开设备")
            self._dev = None
            try:
                dev.close()
            except OSError:
                pass
            self._init_device()
            if self._dev is None:
                return
        except Exception as exc:  # noqa: BLE001 - 恢复失败不能拖垮线程
            self._warn(f"Warning: LSM6DSV16X 就地重初始化失败，改用软复位: {exc}")
            try:
                self._hard_reset(dev)
                self._configure(dev)
            except Exception as exc2:  # noqa: BLE001
                self._warn(f"Warning: LSM6DSV16X 软复位恢复失败: {exc2}")
                return
        now = time.perf_counter()
        self._last_report_s = now
        self._last_quat_s = now
        self._imu_failed = False
        self._stall_reason = ""
        print(
            f"IMU: LSM6DSV16X 已重初始化（第 {self._stall_count} 次恢复尝试）",
            end="\r\n",
            flush=True,
        )
