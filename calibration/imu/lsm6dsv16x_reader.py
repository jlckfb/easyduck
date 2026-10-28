# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Marc Duclusaud
"""LSM register setup retained from the existing reader; sensor-frame output only."""
import math
import struct
import time
from .transport import I2c

def decode_sflp(word):
    if len(word)!=7 or word[0]>>3!=0x13: raise ValueError('invalid SFLP word')
    x,y,z=struct.unpack('<3e',word[1:]); n=x*x+y*y+z*z
    if not math.isfinite(n) or n>1.002: raise ValueError('invalid SFLP quaternion')
    from . import normalized
    return normalized([math.sqrt(max(0,1-n)),x,y,z])

class LsmDriver:
    def __init__(self,c):
        self.bus=I2c(c['i2c_dev'],c['i2c_addr']);self.sflp=c['fusion']=='sflp';self.settled=0
        self.capabilities={'orientation':self.sflp,'gyro':True,'accel':True}
        try: self.initialize()
        except BaseException: self.close();raise
    def initialize(self):
        b=self.bus
        if b.reg(0x0f)[0]!=0x70: raise IOError('LSM WHO_AM_I does not match 0x70')
        for r,v in ((0x12,0x44),(0x15,2),(0x17,1),(0x10,7),(0x11,7)): b.set(r,v)
        if not self.sflp: return
        for r,v in ((7,0),(8,0),(9,0)): b.set(r,v)
        self.bank(True)
        try: b.set(0x44,(b.reg(0x44)[0]|2)&~0x30)
        finally: self.bank(False)
        b.set(0x0a,6);self.bank(True)
        try:
            b.set(0x5e,(b.reg(0x5e)[0]&~0x38)|0x10)
            b.set(4,b.reg(4)[0]|2);b.set(0x66,b.reg(0x66)[0]|2)
        finally: self.bank(False)
    def bank(self,on):
        b=self.bus;v=b.reg(1)[0];b.set(1,(v|0x80) if on else (v&~0x80))
    def read(self):
        b=self.bus;s={};flags=b.reg(0x1e)[0]
        if flags&2: s['gyro']=(tuple(v*.0175*math.pi/180 for v in struct.unpack('<3h',b.reg(0x22,6))),time.monotonic())
        if flags&1: s['accel']=(tuple(v*.000122*9.80665 for v in struct.unpack('<3h',b.reg(0x28,6))),time.monotonic())
        if self.sflp:
            st=b.reg(0x1b,2);pending=st[0]|((st[1]&1)<<8);overloaded=pending>16 or st[1]&0x40
            for _ in range(min(pending,16)):
                word=b.reg(0x78,7)
                if not overloaded and word[0]>>3==0x13:
                    value=decode_sflp(word);self.settled+=1
                    if self.settled>=25:s['quat']=(value,time.monotonic())
            if overloaded:
                from . import BacklogError
                raise BacklogError('LSM FIFO backlog; draining stale samples')
        return s
    def close(self): self.bus.close()
