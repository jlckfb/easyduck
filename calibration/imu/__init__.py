"""Common sensor-frame contract. Times are monotonic seconds; absent data stays absent."""
import copy
import math
import threading
import time
import tomllib
from pathlib import Path
from calibration_math import norm,mul,inv,rotate,cross,gravity,validate_imu

class BacklogError(IOError):pass

MAX_AGE=.15
MODELS={'lsm6dsv16x':((0x6a,0x6b),('sflp','comp')),'bno085':((0x4a,0x4b),('game_rv',))}
def normalized(q):
    n=math.sqrt(sum(x*x for x in q))
    if not math.isfinite(n) or not 1e-9<=n<=10:raise ValueError('invalid quaternion')
    return [x/n for x in q]
def configuration(path):
    c={'source':'bus','model':'lsm6dsv16x','i2c_dev':'/dev/i2c-3','i2c_addr':0x6a,'fusion':'sflp',
       'lever_arm':[-.014,.001,.0567],'g_filter_k':.05,'accel_trust_g':.3}
    with Path(path).open('rb') as f:raw=tomllib.load(f).get('imu',{})
    if set(raw)-set(c):raise ValueError('unknown IMU configuration keys: '+str(sorted(set(raw)-set(c))))
    c.update(raw);validate_config(c);return c
def validate_config(c):
    if any(type(c[k]) is not str for k in ('source','model','i2c_dev','fusion')) or type(c['i2c_addr']) is not int:raise ValueError('invalid IMU configuration types')
    if any(type(c[k]) not in (int,float) for k in ('g_filter_k','accel_trust_g')):raise ValueError('invalid IMU filter types')
    if type(c['lever_arm']) is not list or len(c['lever_arm'])!=3 or any(type(x) not in (int,float) for x in c['lever_arm']):raise ValueError('invalid IMU lever arm types')
    model=MODELS.get(c['model'])
    if c['source'] not in ('bus','i2c') or not model or c['i2c_addr'] not in model[0] or c['fusion'] not in model[1]:raise ValueError('invalid IMU model/address/fusion combination')
    if not isinstance(c['i2c_dev'],str) or not c['i2c_dev'].startswith('/dev/i2c-'):raise ValueError('invalid IMU device')
    if not math.isfinite(c['g_filter_k']) or not 0<=c['g_filter_k']<=1 or not math.isfinite(c['accel_trust_g']) or c['accel_trust_g']<=0:raise ValueError('invalid IMU filter')
    if len(c['lever_arm'])!=3 or not all(math.isfinite(v) for v in c['lever_arm']):raise ValueError('invalid IMU lever arm')

def calibration(profile,model):
    entry=profile.get('imu_calibrations',{}).get(model)
    if entry is None:
        old=profile.get('imu',{})
        if old.get('model','lsm6dsv16x')==model:entry=old
    if entry is None:return {'model':model,'frame':'sensor-v1','mount':[1,0,0,0],'gyro_bias':[0,0,0],'mount_valid':False}
    if entry.get('model',model)!=model:raise ValueError('IMU calibration model mismatch')
    if entry.get('frame','sensor-v1')!='sensor-v1':raise ValueError('unknown IMU calibration frame')
    out=copy.deepcopy(entry);out.setdefault('mount_valid',True);validate_imu(out);return out

def set_calibration(profile,model,**changes):
    cfg=calibration(profile,model);cfg.update(changes,model=model,frame='sensor-v1')
    entries=profile.setdefault('imu_calibrations',{})
    old=profile.get('imu')
    if old:
        old_model=old.get('model','lsm6dsv16x');legacy=copy.deepcopy(old);legacy.update(model=old_model,frame='sensor-v1')
        entries.setdefault(old_model,legacy)
    entries[model]=cfg
    # imu remains the backwards-compatible active entry, while other chip entries survive.
    profile['imu']=copy.deepcopy(cfg)
    return cfg

def body(sample,cfg,now):
    if not cfg.get('mount_valid',True) or any(k not in sample or not 0<=now-sample[k][1]<=MAX_AGE for k in ('quat','gyro')):return None
    q=normalized(mul(sample['quat'][0],inv(cfg['mount'])))
    return {'body_quat':q,'body_gyro':rotate(cfg['mount'],[sample['gyro'][0][i]-cfg['gyro_bias'][i] for i in range(3)]),'gravity':gravity(q)}

class Processor:
    def __init__(self,c,cal):self.c=c;self.cal=cal;self.sample={};self.last=None;self.prev=[0,0,0]
    def update(self,s):
        if 'quat' in s:self.sample['quat']=(normalized(s['quat'][0]),s['quat'][1])
        if 'accel' in s and all(math.isfinite(x) for x in s['accel'][0]):self.sample['accel']=s['accel']
        if 'gyro' not in s or not all(math.isfinite(x) for x in s['gyro'][0]):return
        self.sample['gyro']=s['gyro']
        if self.c['fusion']!='comp':return
        w,at=s['gyro'];a=self.sample.get('accel')
        if not a or not 0<=at-a[1]<=MAX_AGE:return
        dt=0 if self.last is None else at-self.last;self.last=at
        w=[w[i]-self.cal['gyro_bias'][i] for i in range(3)];r=rotate(inv(self.cal['mount']),self.c['lever_arm'])
        alpha=[(w[i]-self.prev[i])/dt for i in range(3)] if 0<dt<.1 else [0,0,0];self.prev=w
        cent=cross(w,cross(w,r));tang=cross(alpha,r);a=[a[0][i]-cent[i]-tang[i] for i in range(3)]
        mag=math.sqrt(sum(x*x for x in a));trusted=mag>1e-6 and abs(mag/9.80665-1)<self.c['accel_trust_g']
        if 'quat' not in self.sample or dt>=.1:
            self.sample.pop('quat',None)
            if trusted:
                u=[x/mag for x in a];q=[0,1,0,0] if u[2]<-.999999 else normalized([1+u[2],u[1],-u[0],0]);self.sample['quat']=(q,at)
            return
        if dt<=0:return
        q=self.sample['quat'][0];correction=cross([x/mag for x in a],rotate(inv(q),[0,0,1])) if trusted else [0,0,0]
        rate=[w[i]+self.c['g_filter_k']*correction[i]/dt for i in range(3)];dq=mul(q,[0]+rate)
        self.sample['quat']=(normalized([q[i]+.5*dt*dq[i] for i in range(4)]),at)

# Registration is intentionally small; no plugin loader or extra service.
def open_driver(c):
    if c['model']=='lsm6dsv16x':
        from .lsm6dsv16x_reader import LsmDriver
        return LsmDriver(c)
    if c['model']=='bno085':
        from .bno085 import BnoDriver
        return BnoDriver(c)
    raise ValueError('unregistered IMU')

class Reader:
    def __init__(self,c,cal):
        validate_config(c);self.c=c;self.processor=Processor(c,cal);self.quit=threading.Event();self.lock=threading.Lock();self.thread=None;self.pending=None;self.history=[];self.history_key=None
        self.state={'sample':{},'capabilities':{'orientation':False,'gyro':False,'accel':False},'connected':False,'error':'initializing'}
    def start(self):
        if self.thread is not None:raise RuntimeError('IMU reader already started')
        self.thread=threading.Thread(target=self.run,name='imu-reader');self.thread.start()
    def set_calibration(self,cal):
        with self.lock:
            if cal!=self.processor.cal:self.pending=copy.deepcopy(cal)
    def history_snapshot(self):
        with self.lock:return copy.deepcopy(self.history)
    def snapshot(self):
        with self.lock:return copy.deepcopy(self.state)
    def run(self):
        d=None
        try:
            d=open_driver(self.c);backoff=.02
            while not self.quit.is_set():
                error=None
                with self.lock:
                    if self.pending is not None:self.processor=Processor(self.c,self.pending);self.pending=None;self.history=[];self.history_key=None
                try:
                    sample=d.read();self.processor.update(sample);backoff=.02 if sample else min(.5,backoff*2)
                except Exception as e:error=str(e);backoff=.02 if isinstance(e,BacklogError) else min(.5,backoff*2)
                with self.lock:
                    current=self.processor.sample
                    self.state={'sample':copy.deepcopy(current),'capabilities':dict(d.capabilities),'connected':error is None and any(0<=time.monotonic()-v[1]<=MAX_AGE for v in current.values()),'error':error}
                    now=time.monotonic()
                    if error is None and all(k in current and 0<=now-current[k][1]<=MAX_AGE for k in ('quat','gyro')):
                        q,w=current['quat'],current['gyro'];key=(q[1],w[1])
                        if self.history_key is None or all(key[i]>self.history_key[i] for i in (0,1)):
                            self.history.append({'quat':list(q[0]),'gyro':list(w[0]),'acc':list(current['accel'][0]) if 'accel' in current else None,
                                'at':min(key),'quat_at':key[0],'gyro_at':key[1]})
                            self.history=self.history[-160:];self.history_key=key
                self.quit.wait(backoff)
        except Exception as e:
            with self.lock:self.state['error']=str(e);self.state['connected']=False
        finally:
            if d:
                try:d.close()
                except Exception as e:
                    with self.lock:self.state['error']=str(e)
            with self.lock:self.state['connected']=False
    def stop(self):
        self.quit.set()
        if self.thread:
            self.thread.join(6)
            if self.thread.is_alive():raise RuntimeError('I2C adapter did not honor bounded timeout; reader still stopping')
