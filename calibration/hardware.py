"""Exclusive SCS serial access. No torque-on or EEPROM writes on construction."""
import os
import json
from pathlib import Path
import select
import struct
import time

def packet(sid,instruction,params):
    b=bytes([sid,len(params)+2,instruction])+bytes(params)
    return b'\xff\xff'+b+bytes([(~sum(b))&255])

def payload(req,p,expected):
    if p==req or len(p)<6 or p[:2]!=b'\xff\xff' or p[2]!=req[2]: return None
    if len(p)!=p[3]+4 or ((~sum(p[2:-1]))&255)!=p[-1]: raise IOError('SCS校验或长度错误')
    if p[4]: raise IOError('舵机ID %s 报警码 %s'%(p[2],p[4]))
    if len(p)==6 and expected: return None
    if len(p)-6!=expected: raise IOError('SCS反馈长度错误')
    return p[5:-1]

class ServoTimeout(TimeoutError):
    """A missing response, distinct from malformed replies and device alarms."""


class Bus:
    def __init__(self,path='/dev/ttyS3',speed_restore_path=None,position_limits=None):
        self.speed_restore_path=Path(speed_restore_path) if speed_restore_path else None
        self.original_speeds={}
        self.position_limits={int(k):list(v) for k,v in (position_limits or {}).items()}
        self.turn_offsets={}
        self.raw_positions={}
        if self.speed_restore_path and self.speed_restore_path.exists():
            saved=json.loads(self.speed_restore_path.read_text(encoding='utf-8'))
            if not isinstance(saved,dict) or any(not k.isdecimal() or not 1<=int(k)<=253
                    or type(v) is not int or not 0<=v<=65535 for k,v in saved.items()):
                raise ValueError('速度恢复记录无效，未打开舵机总线')
            self.original_speeds={int(k):v for k,v in saved.items()}
        import termios,fcntl
        self.t=termios
        self.fd=os.open(path,os.O_RDWR|os.O_NOCTTY|os.O_NONBLOCK)
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        a=termios.tcgetattr(self.fd); a[0]=a[1]=a[3]=0
        a[2]=termios.CS8|termios.CLOCAL|termios.CREAD
        a[4]=a[5]=termios.B1000000
        a[6][termios.VMIN]=0; a[6][termios.VTIME]=0
        termios.tcsetattr(self.fd,termios.TCSANOW,a)

    def read_raw(self,sid,addr,n):
        req=packet(sid,2,[addr,n])
        for _ in range(2):
            self.t.tcflush(self.fd,self.t.TCIOFLUSH); os.write(self.fd,req); self.t.tcdrain(self.fd)
            end=time.monotonic()+.02; buf=b''
            while time.monotonic()<end:
                if not select.select([self.fd],[],[],.002)[0]: continue
                buf+=os.read(self.fd,512)
                while len(buf)>=4:
                    start=buf.find(b'\xff\xff')
                    if start<0: buf=buf[-1:]; break
                    buf=buf[start:]
                    if len(buf)<4: break
                    nbytes=buf[3]+4
                    if nbytes<6: buf=buf[2:]; continue
                    if len(buf)<nbytes: break
                    p,buf=buf[:nbytes],buf[nbytes:]
                    got=payload(req,p,n)
                    if got is not None: return got
        raise ServoTimeout('舵机ID %s读取超时'%sid)

    def write_raw(self,sid,addr,data):
        os.write(self.fd,packet(sid,3,[addr]+list(data))); self.t.tcdrain(self.fd)
        time.sleep(.003)

    @staticmethod
    def signed_position(word):
        return -(word&32767) if word&32768 else word

    @staticmethod
    def position_word(value):
        if abs(value)>32767:raise ValueError('位置超出舵机多圈范围')
        return abs(value)|(32768 if value<0 else 0)

    def read(self,sid,addr,n):
        data=self.read_raw(sid,addr,n)
        if addr==56 and n>=2:
            raw=int.from_bytes(data[:2],'little')
            count=self.signed_position(raw)
            lo,hi=self.position_limits.get(sid,[0,4095])
            # Select the equivalent angle nearest the saved joint travel window.
            import math
            turns=math.floor((count-(lo+hi)/2)/4096+.5)
            logical=count-turns*4096
            self.raw_positions[sid]=raw
            self.turn_offsets[sid]=count-logical
            if not 0<=logical<=4095:raise IOError('位置超出标定单圈范围')
            data=logical.to_bytes(2,'little')+data[2:]
        elif addr==42 and n==2:
            logical=self.signed_position(int.from_bytes(data,'little'))-self.turn_offsets.get(sid,0)
            data=self.position_word(logical).to_bytes(2,'little')
        return data

    def write(self,sid,addr,data):
        if addr==42 and len(data)==2:
            if sid not in self.turn_offsets:self.read(sid,56,2)
            logical=int.from_bytes(bytes(data),'little')
            if not 0<=logical<=4095:raise ValueError('无效单圈目标')
            physical=logical+self.turn_offsets[sid]
            data=self.position_word(physical).to_bytes(2,'little')
        self.write_raw(sid,addr,data)

    def ensure_current_limit(self,sid):
        # Never overwrite a configured nonzero current limit. A former six-byte
        # goal write cleared address 44; recover that case from this servo's EEPROM.
        if self.read(sid,44,2)==bytes(2):
            current=self.read(sid,28,2)
            if not 0<int.from_bytes(current,'little')<=2047: raise IOError('舵机保护电流无效')
            self.write(sid,44,list(current))
            if self.read(sid,44,2)!=current: raise IOError('电流限制恢复回读失败')

    def reset_head_yaw_turns(self,sid):
        raw=getattr(self,'raw_positions',{}).get(sid,0)
        count=self.signed_position(raw)
        if sid!=32 or 0<=count<4096:return
        if self.read(sid,40,1)!=b'\0':raise IOError('ID 32 must be released before turn reset')
        if self.read(sid,9,4)!=bytes([0,0,255,15]):raise IOError('ID 32 turn reset requires single-turn limits')
        # RESET clears volatile turns; it does not recalibrate or rewrite EEPROM.
        os.write(self.fd,packet(sid,0x0a,[]));self.t.tcdrain(self.fd);time.sleep(.003)
        after=self.signed_position(int.from_bytes(self.read_raw(sid,56,2),'little'))
        if not 0<=after<4096 or abs(after-count%4096)>8 or self.read(sid,40,1)!=b'\0':
            raise IOError('ID 32 turn reset verification failed')
        self.turn_offsets[sid]=0
        self.raw_positions[sid]=after

    def torque(self,sid,on):
        if on:
            # Capture target while still released. Never revive a stale policy goal.
            if self.read(sid,40,1)[0] != 0: raise IOError('实际已上力，请刷新状态')
            p=self.read(sid,56,2)
            if int.from_bytes(p,'little')>4095: raise IOError('位置读数超出单圈范围')
            # This board's HD1910 servos use position mode 4; never rewrite mode.
            if self.read(sid,33,1) not in (b'\0',b'\4'): raise IOError('舵机不是位置模式，不能上力保持位置')
            self.reset_head_yaw_turns(sid)
            p=self.read(sid,56,2)
            self.ensure_current_limit(sid)
            if self.read(sid,46,2)==bytes(2):
                self.set_speed(sid,1)
            self.write(sid,50,[10,20])
            if self.read(sid,50,2)!=bytes([10,20]): raise IOError('增益回读失败')
            self.write(sid,42,list(p))
            if self.read(sid,42,2)!=p: raise IOError('当前位置保持目标回读失败')
            self.write(sid,40,[1])
        else:
            self.write(sid,40,[0])
        state=self.read(sid,40,1)[0]
        if state!=int(on): raise IOError('力矩状态回读不一致')
        return bool(state)

    def set_speed(self,sid,units):
        self.remember_speed(sid)
        self.ensure_current_limit(sid)
        data=struct.pack('<H',units)
        self.write(sid,46,list(data))
        if self.read(sid,46,2)!=data: raise IOError('运动速度回读失败')

    def save_speed_restore(self):
        path=getattr(self,'speed_restore_path',None)
        if path is None:return
        path.parent.mkdir(parents=True,exist_ok=True)
        tmp=path.with_name(path.name+'.tmp')
        with tmp.open('w',encoding='utf-8') as f:
            json.dump(self.original_speeds,f,sort_keys=True)
            f.flush();os.fsync(f.fileno())
        os.replace(tmp,path)

    def remember_speed(self,sid):
        if not hasattr(self,'original_speeds'):self.original_speeds={}
        if sid not in self.original_speeds:
            self.original_speeds[sid]=int.from_bytes(self.read(sid,46,2),'little')
            try:self.save_speed_restore()
            except Exception:
                del self.original_speeds[sid]
                raise

    def restore_speeds(self):
        # Restore only after torque-off is independently read back. Never touch
        # a target or turn torque on here; keep failed entries for the next exit.
        failures={}
        for sid,units in list(getattr(self,'original_speeds',{}).items()):
            try:
                if self.read(sid,40,1)!=b'\0':raise IOError('未确认松力，未恢复速度')
                data=struct.pack('<H',units)
                self.write(sid,46,list(data))
                if self.read(sid,46,2)!=data:raise IOError('速度恢复回读失败')
                del self.original_speeds[sid]
                try:self.save_speed_restore()
                except Exception:
                    self.original_speeds[sid]=units
                    raise
            except Exception as e:failures[sid]=str(e)
        return failures

    def goal(self,sid,count):
        if type(count) is not int or not 0<=count<=4095: raise ValueError('无效目标计数')
        data=struct.pack('<H',count)
        self.write(sid,42,list(data))
        if self.read(sid,42,2)!=data: raise IOError('运动目标回读失败')

    def close(self): os.close(self.fd)

class MockBus:
    def __init__(self,profile):
        self.pos={int(k):v for k,v in profile['zero'].items()}; self.on={i:False for i in self.pos}; self.goals=[];self.speeds={}
    def read(self,sid,addr,n):
        if addr==40: return bytes([int(self.on[sid])])
        if addr==33: return bytes([0])
        if addr==56: return struct.pack('<H',self.pos[sid])+bytes(max(0,n-2))
        return bytes(n)
    def read_raw(self,sid,addr,n):
        if sid not in self.pos: raise ServoTimeout('模拟舵机无响应')
        if addr==5: return bytes([sid])
        if addr==55: return bytes([getattr(self,'locks',{}).get(sid,1)])
        return self.read(sid,addr,n)

    def write_raw(self,sid,addr,data):
        if sid not in self.pos: return
        if addr==5:
            new=data[0];self.pos[new]=self.pos.pop(sid);self.on[new]=self.on.pop(sid)
            if hasattr(self,'locks') and sid in self.locks:self.locks[new]=self.locks.pop(sid)
        elif addr==55:
            if not hasattr(self,'locks'):self.locks={}
            self.locks[sid]=data[0]
        elif addr==40:self.on[sid]=bool(data[0])
        else:raise ValueError('模拟 ID 设置禁止此写入')

    def torque(self,sid,on): self.on[sid]=on; return on
    def set_speed(self,sid,units): self.speeds[sid]=units
    def goal(self,sid,count): self.goals.append((sid,count)); self.pos[sid]=count
    def restore_speeds(self):return {}
    def close(self): pass
