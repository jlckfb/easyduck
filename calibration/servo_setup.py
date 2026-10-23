"""Selected-servo ID provisioning; caller serializes every operation with the bus lock."""
import copy
import json
import os
from pathlib import Path
import time
from hardware import ServoTimeout


def valid_id(value):
    if type(value) is not int or not 0 <= value <= 253:
        raise ValueError('ID 必须是 0～253 的整数，254/255 为保留地址')
    return value


class ServoSetup:
    def __init__(self, bus, journal=None):
        self.bus, self.journal = bus, Path(journal) if journal else None
        self.active = False
        self.scanning = False
        self.next_id = 0
        self.found = []
        self.selected = None
        self.error = None

    def state(self):
        return copy.deepcopy(dict(active=self.active, scanning=self.scanning,
            checked=self.next_id, total=254, found=self.found, selected=self.selected,
            error=self.error, complete=self.next_id == 254))

    def probe(self, sid):
        try:
            value = self.bus.read_raw(sid, 5, 1)
        except ServoTimeout:
            return False
        if value != bytes([sid]):
            raise IOError('ID %s 的地址寄存器与回复不一致，已停止操作' % sid)
        return True

    def scan(self):
        if not self.active: raise ValueError('请先进入 ID 分配模式')
        self.found = []; self.selected = None; self.error = None
        self.next_id = 0; self.scanning = True

    def tick(self):
        if not self.scanning: return
        sid = self.next_id
        try:
            if self.probe(sid): self.found.append(sid)
            self.next_id += 1
            if self.next_id == 254: self.scanning = False
        except Exception as error:
            self.scanning = False; self.error = str(error)

    def require_idle(self):
        if not self.active: raise ValueError('请先进入 ID 分配模式')
        if self.scanning: raise ValueError('请先停止扫描或等待扫描完成')

    def select(self, sid):
        self.require_idle(); valid_id(sid)
        if sid not in self.found: raise ValueError('请先扫描并选择已发现的 ID')
        if not self.probe(sid): raise IOError('该舵机已离线，请重新扫描')
        torque = self.bus.read_raw(sid, 40, 1)[0]
        if torque not in (0, 1): raise IOError('力矩状态无效')
        self.selected = dict(id=sid, torque=bool(torque),
            position_raw=int.from_bytes(self.bus.read_raw(sid, 56, 2), 'little'))
        return self.selected

    def relax(self, sid):
        self.select(sid)
        self.bus.write_raw(sid, 40, [0])
        if self.bus.read_raw(sid, 40, 1) != b'\0': raise IOError('未回读确认断力')
        return self.select(sid)

    def record(self, data):
        if self.journal:
            self.journal.parent.mkdir(parents=True, exist_ok=True)
            fd=os.open(self.journal, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
            with os.fdopen(fd, 'a', encoding='utf-8') as stream:
                stream.write(json.dumps(dict(time=time.time(), **data), ensure_ascii=False)+'\n')
                stream.flush(); os.fsync(stream.fileno())

    def change(self, sid, new_id, single_confirmed):
        self.require_idle(); valid_id(sid); valid_id(new_id)
        if single_confirmed is not True:
            raise ValueError('请确认所选 ID 只对应一只舵机；重复 ID 无法通过扫描区分')
        if sid not in self.found or self.error:
            raise ValueError('请重新扫描并选择要改号的舵机')
        self.select(sid)
        if self.selected['torque']: raise ValueError('请先松开所选舵机力矩')
        if sid == new_id: return {'message':'新旧 ID 相同，未写入'}
        if self.probe(new_id): raise ValueError('目标 ID 已被占用，未写入')
        original_lock = self.bus.read_raw(sid, 55, 1)
        if original_lock not in (b'\0', b'\1'): raise IOError('存储锁状态无效，未写入')
        self.record(dict(event='before', old_id=sid, new_id=new_id, original_lock=original_lock[0]))
        # Address 5 changes identity. Do not write position, gains, limits or center.
        result_error = None
        verified = False
        try:
            for _ in range(3):
                self.bus.write_raw(sid, 55, [0]); time.sleep(.01)
            if self.bus.read_raw(sid,55,1) != b'\0': raise IOError('存储锁未解开')
            for _ in range(3):
                self.bus.write_raw(sid, 5, [new_id]); time.sleep(.01)
            if not self.probe(new_id): raise IOError('写入后新 ID 无响应，请重新扫描')
            if self.probe(sid): raise IOError('新旧 ID 同时有响应，请断开其他舵机并重新扫描')
            if self.bus.read_raw(new_id,40,1) != b'\0': raise IOError('新 ID 的断力状态未确认')
            verified = True
        except Exception as error:
            result_error = error
        finally:
            # Restore only a uniquely identified responder; never guess on ambiguity.
            try:
                responders = [i for i in (sid,new_id) if self.probe(i)]
                if len(responders) != 1: raise IOError('无法唯一确认当前 ID，未恢复存储锁，请重新扫描核对')
                current=responders[0]
                for _ in range(3):
                    self.bus.write_raw(current,55,original_lock); time.sleep(.01)
                if self.bus.read_raw(current,55,1) != original_lock: raise IOError('存储锁恢复未确认')
            except Exception as error:
                result_error=IOError('%s；%s' % (result_error or 'ID 已写入',error))
            self.found=[]; self.selected=None; self.next_id=0; self.error=None
            self.record(dict(event='after',old_id=sid,new_id=new_id,verified=verified and result_error is None,
                             error=str(result_error) if result_error else None))
        if result_error: raise result_error
        return {'message':'ID %s → %s 已回读确认，舵机保持断力；请重新扫描后再操作' % (sid,new_id)}
