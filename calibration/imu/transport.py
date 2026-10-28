"""Exact, bounded Linux i2c-dev transactions, without forced ownership."""
import os
class I2c:
    def __init__(self,dev,addr):
        import fcntl
        self.fd=os.open(dev,os.O_RDWR|os.O_CLOEXEC)
        try:
            for op,arg in ((0x0702,10),(0x0701,0),(0x0703,addr)): fcntl.ioctl(self.fd,op,arg)
        except BaseException:
            self.close(); raise
    def write(self,data):
        if os.write(self.fd,bytes(data))!=len(data): raise IOError('short I2C write')
    def read(self,n):
        b=os.read(self.fd,n)
        if len(b)!=n: raise IOError('short I2C read')
        return b
    def reg(self,r,n=1): self.write([r]); return self.read(n)
    def set(self,r,v): self.write([r,v])
    def close(self):
        if self.fd is not None: os.close(self.fd); self.fd=None
