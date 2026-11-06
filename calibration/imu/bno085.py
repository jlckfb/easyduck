"""Independent SH-2/SHTP implementation; protocol references: docs/en/imu-drivers.md."""
import struct
import time
from .transport import I2c

class Reports:
    def __init__(self):self.seq={}
    def parse(self,p,received):
        from . import normalized
        seq=self.seq.copy();out={};i=0;reference=0
        while i<len(p):
            rid=p[i];n={0xfb:5,0xfa:5,0xef:2,1:10,2:10,8:12}.get(rid)
            if n is None:raise ValueError('unknown SH-2 sensor report')
            if i+n>len(p):raise ValueError('truncated SH-2 report')
            v=p[i:i+n];i+=n
            if rid==0xfb:reference=-int.from_bytes(v[1:5],'little');continue
            if rid==0xfa:reference+=int.from_bytes(v[1:5],'little',signed=True);continue
            if rid==0xef:continue
            offset=(reference+((v[2]&0xfc)<<6)+v[3])*.0001
            if offset>0 or received+offset<0:raise ValueError('invalid SH-2 timestamp')
            at=received+offset;old=seq.get(rid);duplicate=old is not None and ((v[1]-old)&255) not in range(1,128)
            if duplicate:continue
            seq[rid]=v[1]
            if v[2]&3==0:continue
            if rid==8:
                x,y,z,w=struct.unpack('<4h',v[4:12]);q=[w/16384,x/16384,y/16384,z/16384]
                if abs(sum(x*x for x in q)-1)>.02:raise ValueError('invalid BNO quaternion norm')
                out['quat']=(normalized(q),at)
            else:out['gyro' if rid==2 else 'accel']=(tuple(x/(512 if rid==2 else 256) for x in struct.unpack('<3h',v[4:10])),at)
        self.seq=seq;return out

class BnoDriver:
    def __init__(self,c):
        self.bus=I2c(c['i2c_dev'],c['i2c_addr']);self.tx=[0]*6;self.rx=[None]*6;self.reports=Reports();self.features={}
        self.capabilities={'orientation':False,'gyro':False,'accel':False}
        try:self.initialize()
        except BaseException:self.close();raise
    def send(self,ch,p):
        self.bus.write(struct.pack('<HBB',len(p)+4,ch,self.tx[ch])+bytes(p));self.tx[ch]=(self.tx[ch]+1)&255
    def packet(self):
        try:h=self.bus.read(4)
        except OSError as e:
            if e.errno==121:return None
            raise
        raw,ch,seq=struct.unpack('<HBB',h);n=raw&0x7fff
        if n==0:return None
        if not 4<=n<=4096 or ch>=6:raise IOError('invalid SHTP header')
        p=self.bus.read(n)
        if raw&0x8000:raise IOError('orphan SHTP continuation')
        # Reading only the header starts a transfer with no payload. BNO085
        # continues it with the same remaining length and the next sequence.
        tail,ch2,seq2=struct.unpack('<HBB',p[:4])
        if p[:4]!=h and not (tail==(raw|0x8000) and ch2==ch and seq2==((seq+1)&255)):
            raise IOError('SHTP continuation does not match header')
        seq=seq2
        if self.rx[ch] is not None and ((seq-self.rx[ch])&255) not in range(1,128):return None
        self.rx[ch]=seq;return ch,p[4:]
    def initialize(self):
        self.send(2,[0xf9,0]);deadline=time.monotonic()+2;product=False
        while time.monotonic()<deadline:
            packet=self.packet()
            if packet and packet[0]==2 and packet[1][:1]==bytes([0xf8]):
                p=packet[1]
                if len(p)%16 or any(p[i]!=0xf8 for i in range(0,len(p),16)):raise IOError('truncated Product ID')
                product=True;break
            time.sleep(.02)
        if not product:raise TimeoutError('BNO085 Product ID timeout')
        for rid in (8,2,1):
            p=bytearray(17);p[0:2]=bytes([0xfd,rid]);p[5:9]=struct.pack('<I',20000);self.send(2,p);self.send(2,[0xfe,rid])
        deadline=time.monotonic()+2
        while time.monotonic()<deadline and len(self.features)<3:
            packet=self.packet()
            if packet and packet[0]==2:self.control(packet[1])
            time.sleep(.02)
        if not self.features.get(8) or not self.features.get(2):raise IOError('BNO085 did not enable orientation and calibrated gyro reports')
        self.capabilities={'orientation':True,'gyro':True,'accel':bool(self.features.get(1))}
    def control(self,p):
        i=0
        while i<len(p):
            n={0xfc:17,0xf8:16,0xf1:16}.get(p[i])
            if n is None:raise ValueError('unknown BNO control report')
            if i+n>len(p):raise ValueError('truncated BNO control report')
            if p[i]==0xfc and p[i+1] in (8,2,1):self.features[p[i+1]]=int.from_bytes(p[i+5:i+9],'little')>0
            i+=n
    def read(self):
        out={}
        for _ in range(8):
            received=time.monotonic();packet=self.packet()
            if not packet:return out
            ch,p=packet
            if ch in (3,4):out.update(self.reports.parse(p,received))
            if ch==2:self.control(p)
            if ch==1:raise IOError('BNO085 reset; restart reader after checking hardware')
        from . import BacklogError
        raise BacklogError('SHTP backlog; draining stale reports')
    def close(self):self.bus.close()
