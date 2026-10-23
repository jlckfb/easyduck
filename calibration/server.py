#!/usr/bin/env python3
"""LAN-only visual calibration service; all hardware access is serialized."""
import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit
from calibration_math import *
from hardware import Bus,MockBus
from servo_setup import ServoSetup
from motion import MotionCoordinator,COUNT_DEG

ROOT=Path(__file__).resolve().parent
MODEL_SHA=json.loads((ROOT/'static/model/manifest.json').read_text(encoding='utf-8'))['source_sha256']

# robotd builds known to load /etc/robot/visual_calibration.json at startup.
CALIBRATION_READERS={'6cddf3a2b0bcbbc71dc783fb1b0312849d2f69b5bb223090414cac309827f1d2',
                     '99bcb02b2bea293159b94ea0cd7bee971497057cf2b9bd749d7a9592ca6a6eff'}
CALIBRATION_MARKER=b'cannot read visual calibration'
APPLY_UNITS=('xgoduck-calibration.service','robotd.service','xgoduck-web.service','padd.service')
_runtime_cache={}

def runtime_support(path):
    """Check the installed robotd itself; the marker file is static and not trusted."""
    try:
        st=os.stat(path); key=(path,st.st_mtime_ns,st.st_size)
    except OSError as e:
        return {'supported':False,'sha256':None,'reason':'无法读取正式程序：%s'%e,'path':path}
    if key not in _runtime_cache:
        data=Path(path).read_bytes(); sha=hashlib.sha256(data).hexdigest()
        if sha in CALIBRATION_READERS: reason='已知会读取网页标定的版本'
        elif CALIBRATION_MARKER in data: reason='程序包含读取网页标定的代码'
        else: reason=None
        _runtime_cache.clear()
        _runtime_cache[key]={'supported':reason is not None,'sha256':sha,'path':path,
                             'reason':reason or '当前正式程序不读网页标定'}
    return dict(_runtime_cache[key])

def apply_command():
    # The calibration service cannot stop itself and keep going, so a transient unit runs the chain.
    # Restore the current deployment services stopped by calibration.
    chain=' && '.join('systemctl %s %s'%('stop' if i==0 else 'start',u) for i,u in enumerate(APPLY_UNITS))
    return ['systemd-run','--no-block','--collect','--unit=xgoduck-calibration-apply-%d'%time.time(),
            '/bin/sh','-c',chain]

def reference_aligned(profile):
    mark=profile.get('reference_pose',{})
    return isinstance(mark,dict) and mark.get('model_sha256')==MODEL_SHA and mark.get('angles')==HOME

def atomic_save(path,data):
    path=Path(path); path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists():
        backup=path.parent/'calibration-backups'
        backup.mkdir(exist_ok=True)
        dest=backup/(path.name+'.'+str(time.time_ns())+'.json')
        dest.write_bytes(path.read_bytes())
    temp=path.with_name(path.name+'.tmp')
    with temp.open('w',encoding='utf-8') as f:
        json.dump(data,f,ensure_ascii=False,indent=2,allow_nan=False); f.flush(); os.fsync(f.fileno())
    os.replace(temp,path)

class App:
    def __init__(self,args):
        self.args=args; self.lock=threading.RLock(); self.stop=threading.Event(); self.leaving=False
        baseline=json.loads(Path(args.baseline).read_text(encoding='utf-8'))
        baseline['imu']={'mount':DEFAULT_MOUNT,'gyro_bias':[0,0,0]}
        self.saved=json.loads(Path(args.profile).read_text()) if Path(args.profile).exists() else baseline
        validate_profile(self.saved)
        self.profile=copy.deepcopy(self.saved); self.revision=0
        self.rows={}; self.imu=None; self.samples={}; self.history=[]; self.hz=0
        self.bus=MockBus(self.profile) if args.mock else Bus(args.port,
            speed_restore_path='/etc/robot/calibration_speed_restore.json', position_limits=self.saved['lim'])
        if args.mock and getattr(args,'mock_servo_id',None) is not None:
            self.bus.pos={args.mock_servo_id:2048};self.bus.on={args.mock_servo_id:False}
        self.id_setup=ServoSetup(self.bus,None if args.mock else getattr(args,'id_journal','/var/lib/easyduck/servo-id-changes.jsonl'))
        self.motion=MotionCoordinator(self.bus)
        self.reader=None
        if not args.mock:
            sys.path.insert(0,str(ROOT/'imu'))
            from lsm6dsv16x_reader import Lsm6dsv16xReader
            self.reader=Lsm6dsv16xReader(2,50,delta_quat=(1,0,0,0)); self.reader.start()
        self.thread=threading.Thread(target=self.poll,daemon=True); self.thread.start()

    def read_row(self,sid):
        try:
            b=self.bus.read(sid,56,15)
            p=int.from_bytes(b[:2],'little')
            if not 0<=p<=4095: raise IOError('位置超出单圈范围')
            torque=self.bus.read(sid,40,1)[0]
            if torque not in (0,1): raise IOError('力矩寄存器不是0/1')
            raw=int.from_bytes(b[13:15],'little')
            current=-(raw&32767) if raw&32768 else raw
            raw_position=getattr(self.bus,'raw_positions',{}).get(sid,p)
            self.rows[sid]={'count':p,'position_raw':raw_position,'position_signed':Bus.signed_position(raw_position),
                'torque':bool(torque),'voltage':b[6]/10,'temperature':b[7],
                'current_raw':current,'current_ma':current*6.5,'at':time.monotonic(),'error':None}
        except Exception as e:
            self.rows[sid]={'count':None,'torque':None,'at':time.monotonic(),'error':str(e)}

    def poll(self):
        while not self.stop.is_set():
            start=time.monotonic()
            with self.lock:
                id_mode=self.id_setup.active
                if id_mode:self.id_setup.tick()
                job=self.motion.job if not id_mode else None
                ids=(job.get('ids') or [job['id']]) if job else list(IDS)
            if id_mode:
                self.stop.wait(.005)
                continue
            for sid in ids:
                with self.lock:
                    if self.id_setup.active or self.stop.is_set():break
                    # During motion, service only its members; slow unrelated
                    # servos must not delay the selected joint's trajectory.
                    if job and not self.motion.owns(sid):continue
                    self.read_row(sid)
                    if self.motion.owns(sid):self.motion.tick(self.rows,only=sid)
                # Yield between transactions, even when a round overruns.
                self.stop.wait(.001)
            with self.lock:
                if self.reader:
                    sample=self.reader.get_latest();age=time.perf_counter()-sample.timestamp_s
                    if sample.valid and 0<=age<.3:
                        self.imu={'quat':list(sample.quat),'gyro':list(sample.gyro),'acc':list(sample.acc),'at':time.monotonic()}
                        self.history.append(copy.deepcopy(self.imu));self.history=self.history[-160:]
                    else:self.imu=None
                elif self.args.mock:
                    self.imu={'quat':list(DEFAULT_MOUNT),'gyro':[0,0,0],'acc':[1,0,0],'at':time.monotonic()}
                    self.history.append(copy.deepcopy(self.imu));self.history=self.history[-160:]
            elapsed=time.monotonic()-start
            self.hz=1/max(.06,elapsed)
            self.stop.wait(max(.001,.06-elapsed))

    def state(self):
        with self.lock:
            now=time.monotonic(); rows=[]; aligned=reference_aligned(self.profile)
            for i,sid in enumerate(IDS):
                k=str(sid); r=copy.deepcopy(self.rows.get(sid,{})); age=now-r.pop('at',0)
                if age>.4: r.update(count=None,torque=None,error='反馈已过期')
                r.update(id=sid,name=NAMES[i],label=LABELS[i],age_ms=round(age*1000),
                         angle=None if not aligned or r.get('count') is None else angle(HOME[i],r['count'],self.profile['zero'][k],self.profile['sign'][k]))
                rows.append(r)
            imu=None
            if self.imu and now-self.imu['at']<.3:
                imu=copy.deepcopy(self.imu); cfg=self.profile['imu']
                imu['body_quat']=norm(mul(imu['quat'],inv(cfg['mount'])))
                imu['gravity']=gravity(imu['body_quat'])
                imu['body_gyro']=rotate(cfg['mount'],[imu['gyro'][i]-cfg['gyro_bias'][i] for i in range(3)])
                imu['age_ms']=round((now-imu.pop('at'))*1000)
            return {'rows':rows,'imu':imu,'profile':copy.deepcopy(self.profile),'revision':self.revision,'hz':round(self.hz,1),
                    'mock':self.args.mock,'poses':copy.deepcopy(self.samples),'dirty':self.profile!=self.saved,
                    'runtime_support':runtime_support(self.args.robotd),
                    'profile_path':self.args.profile,'home':dict(zip(NAMES,HOME)),
                    'reference_aligned':aligned,
                    'motion':copy.deepcopy(self.motion.status),'id_setup':self.id_setup.state()}

    def command(self,d):
        with self.lock:
            if not isinstance(d,dict): raise ValueError('请求必须是对象')
            op=d.get('op')
            if op=='cancel_exit':
                self.leaving=False; return {'message':'已取消页面切换'}
            if self.leaving: raise ValueError('正在退出标定，请等待页面切换完成')
            if op=='id_enter':
                if self.motion.job: raise ValueError('请先停止实物运动')
                if self.profile!=self.saved: raise ValueError('请先保存或撤销标定草稿')
                if getattr(self.bus,'original_speeds',{}): raise ValueError('仍有临时速度待恢复，请先在原舵机上完成断力和速度恢复，再分配 ID')
                self.id_setup.active=True
                return {'message':'已进入 ID 分配模式；暂停标定采样和运动，已编号舵机可保留连接，每次只新增一只待编号舵机'}
            if op=='id_exit':
                self.id_setup.require_idle()
                for sid in self.id_setup.found:
                    if self.bus.read_raw(sid,40,1)!=b'\0': raise ValueError('请先确认发现的舵机全部断力')
                self.id_setup.active=False;self.rows={}
                return {'message':'已返回姿态标定；新机器人请重新记录 HOME、方向、限位和 IMU'}
            if op=='id_scan':
                self.id_setup.scan();return {'message':'正在扫描 ID 0～253，不修改舵机参数'}
            if op=='id_stop':
                self.id_setup.scanning=False;return {'message':'已停止扫描，保留已发现的 ID'}
            if op in ('id_select','id_relax'):
                result=(self.id_setup.select if op=='id_select' else self.id_setup.relax)(d.get('id'))
                return {'message':'ID %s 已%s'%(result['id'],'确认断力' if not result['torque'] else '读取，当前仍上力'),'servo':result}
            if op=='id_change':
                return self.id_setup.change(d.get('id'),d.get('new_id'),d.get('single_confirmed'))
            if self.id_setup.active: raise ValueError('ID 分配模式下禁止运动和标定保存，请先返回姿态标定')
            if op=='reset_turns':
                if self.motion.job: raise ValueError('请先停止运动，再清除圈数')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新后重试')
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                if d.get('confirmed') is not True: raise ValueError('请确认仅清除当前关节累计圈数，不设置硬件中位')
                if self.bus.read_raw(sid,40,1)!=b'\0': raise ValueError('请先松开此关节的力矩，再清除圈数')
                result=self.bus.reset_position_turns(sid)
                self.read_row(sid)
                return {'message':'ID%s 累计位置 %s → %s；%s，保持断力，HOME、方向、范围和硬件中位均未修改'%(sid,result['before'],result['after'],'圈数已清除' if result['changed'] else '无需清圈'),'result':result}
            if op=='center_servo':
                if self.motion.job: raise ValueError('请先停止运动，再设置硬件中位')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新后重试')
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                if d.get('confirmed') is not True: raise ValueError('请确认已将关节摆到正确的安装中位；此操作改变舵机永久中位')
                if sid not in self.profile.get('limits_pending',[]): raise ValueError('此关节已有已确认限位；请先按新机器人流程清除旧限位')
                before=self.bus.center_snapshot(sid)
                record={'id':sid,'before':before,'saved_profile':copy.deepcopy(self.saved),'draft_profile':copy.deepcopy(self.profile)}
                journal=Path(self.args.profile).parent/'calibration-backups'/('servo-center-%s-%s.json'%(sid,time.time_ns()))
                atomic_save(journal,record)
                try:
                    result=self.bus.set_center(sid,before)
                except Exception as e:
                    record['error']=str(e);atomic_save(journal,record)
                    raise
                record['result']=result;atomic_save(journal,record)
                saved=copy.deepcopy(self.saved);draft=copy.deepcopy(self.profile)
                for profile in (saved,draft):
                    profile['zero'][str(sid)]=result['position']
                    profile['lim'][str(sid)]=[0,4095]
                    profile['limits_pending']=list(dict.fromkeys(profile.get('limits_pending',[])+[sid]))
                    profile.pop('reference_pose',None)
                    validate_profile(profile)
                atomic_save(self.args.profile,saved)
                self.saved=saved;self.profile=draft;self.revision+=1
                return {'message':'ID%s 硬件中位已回读确认：%s，保持断力；请重新记录整机HOME后再使用实时3D。未修改其他舵机、P/D或限流'%(sid,result['position']),'result':result}
            if op=='reset_limits':
                if self.motion.job: raise ValueError('请先停止运动，再清除旧限位')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新后重试')
                if d.get('confirmed') is not True: raise ValueError('请确认这是新机器人，需要清除旧限位')
                saved=copy.deepcopy(self.saved)
                saved['lim']={str(sid):[0,4095] for sid in IDS}
                saved['limits_pending']=list(IDS)
                validate_profile(saved)
                atomic_save(self.args.profile,saved)
                self.saved=saved
                self.profile['lim']=copy.deepcopy(saved['lim'])
                self.profile['limits_pending']=list(IDS)
                self.revision+=1
                return {'message':'旧限位已清除并备份；方向和HOME草稿保留。0～4095只是编码器范围，未确认机械限位前不能运行上下限测试'}
            if op=='confirm_limits':
                if self.motion.job: raise ValueError('请先停止运动，再确认限位')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新后重试')
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                validate_profile(self.profile)
                self.profile['limits_pending']=[i for i in self.profile.get('limits_pending',[]) if i!=sid]
                self.revision+=1
                return {'message':'ID%s 当前限位已确认到草稿，请保存标定后使用'%sid}
            if op=='capture_home_one':
                if self.motion.job: raise ValueError('请先停止运动，再记录HOME')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新后重试')
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                p=int.from_bytes(self.bus.read(sid,56,2),'little')
                lo,hi=self.profile['lim'][str(sid)]
                if not 0<=p<=4095 or not lo<=p<=hi: raise ValueError('当前位置超出当前范围，请先核对该关节限位')
                profile=copy.deepcopy(self.profile);profile['zero'][str(sid)]=p
                validate_profile(profile)
                self.profile=profile;self.revision+=1
                return {'message':'ID%s 的当前位置 %s 已记为HOME草稿；点击保存标定后生效，舵机不动作'%(sid,p),'position':p}
            if op=='capture_home_all':
                if self.motion.job: raise ValueError('请先停止运动，再记录全部HOME')
                if d.get('revision')!=self.revision: raise ValueError('标定已变化，请刷新后重试')
                if self.profile!=self.saved: raise ValueError('请先保存或撤销已有草稿，再记录全部HOME')
                captured={}
                for sid in IDS:
                    p=int.from_bytes(self.bus.read(sid,56,2),'little')
                    lo,hi=self.saved['lim'][str(sid)]
                    if not 0<=p<=4095: raise IOError('ID%s 当前位置无效，未保存任何HOME'%sid)
                    if not lo<=p<=hi: raise ValueError('ID%s 当前位置超出所设上下限，未保存任何HOME'%sid)
                    captured[str(sid)]=p
                saved=copy.deepcopy(self.saved)
                saved['zero']=captured
                saved['reference_pose']={'model_sha256':MODEL_SHA,'angles':list(HOME)}
                saved['saved_at']=time.strftime('%Y-%m-%d %H:%M:%S')
                validate_profile(saved)
                atomic_save(self.args.profile,saved)
                self.saved=saved;self.profile=copy.deepcopy(saved);self.revision+=1
                return {'message':'15个当前位置已保存为HOME，已启用实时对比；舵机未执行运动'}
            if op=='stop_motion':
                return self.motion.cancel()
            if op=='off_all':
                self.motion.cancel('已取消运动并松力矩',release=True)
                failures={}
                for sid in IDS:
                    try: self.bus.torque(sid,False)
                    except Exception as e: failures[sid]=str(e)
                if failures: raise IOError('部分未确认断力：'+json.dumps(failures,ensure_ascii=False))
                return {'message':'15个舵机已回读确认松力矩'}
            if op=='on_all':
                if self.motion.job: raise ValueError('请先停止运动，再全部上力')
                try:
                    for sid in IDS:
                        actual=self.bus.read(sid,40,1)[0]
                        if actual not in (0,1): raise IOError('ID%s 力矩状态未知'%sid)
                        if actual==0:self.bus.torque(sid,True)
                        else:
                            current=int.from_bytes(self.bus.read(sid,56,2),'little')
                            if not 0<=current<=4095:raise IOError('ID%s 位置读数无效'%sid)
                            self.bus.goal(sid,current)
                    for sid in IDS:
                        if self.bus.read(sid,40,1)[0]!=1:raise IOError('ID%s 未回读确认上力'%sid)
                except Exception as e:
                    failures=[]
                    for sid in IDS:
                        try:self.bus.torque(sid,False)
                        except Exception:failures.append(sid)
                    raise IOError('全部上力失败，已逐个尝试断力：%s；断力未确认：%s'%(e,failures)) from e
                return {'message':'15个舵机已上力并保持各自当前位置，已逐个回读确认'}
            if op=='home_one':
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS:raise ValueError('无效舵机ID')
                if d.get('revision')!=self.revision:raise ValueError('草稿已变化，请刷新后重试')
                key=str(sid)
                if any(self.profile[field][key]!=self.saved[field][key] for field in ('zero','sign','lim')) or ((sid in self.profile.get('limits_pending',[]))!=(sid in self.saved.get('limits_pending',[]))):
                    raise ValueError('请先保存当前关节或撤销该关节的修改，再回已保存 HOME')
                target=self.saved['zero'][key]
                lo,hi=self.saved['lim'][key]
                if not 0<=target<=4095 or not lo<=target<=hi:raise ValueError('已保存 HOME 超出当前关节活动范围，请先核对标定')
                return self.motion.start(sid,[target],d.get('speed',5),'home',['ID%s 正在缓慢回已保存 HOME'%sid])
            if op=='home':
                if d.get('revision')!=self.revision:raise ValueError('草稿已变化，请刷新后重试')
                if self.profile!=self.saved:raise ValueError('请先保存标定或撤销草稿，再回已保存的HOME')
                targets={sid:self.profile['zero'][str(sid)] for sid in IDS}
                return self.motion.start_home(targets,d.get('speed',5))
            if op=='torque':
                sid=d['id']
                if type(sid) is not int or sid not in IDS or type(d['on']) is not bool: raise ValueError('无效力矩请求')
                if not d['on'] and self.motion.owns(sid):
                    self.motion.cancel('已取消运动并松力矩',release=True)
                actual=self.bus.read(sid,40,1)[0]
                if actual not in (0,1): raise IOError('实际力矩状态未知')
                if d['on'] and actual!=0: raise IOError('状态已改变，请刷新后重试')
                try: state=self.bus.torque(sid,d['on'])
                except Exception:
                    if d['on']:
                        try: self.bus.torque(sid,False)
                        except Exception: pass
                    self.rows[sid]={'count':None,'torque':None,'at':time.monotonic(),'error':'力矩操作失败，需要重新回读'}
                    raise
                self.rows.pop(sid,None)
                return {'message':'ID%s 已%s力矩并回读确认'%(sid,'上' if state else '松'),'torque':state}
            if op in ('step','test_limits'):
                sid=d['id']
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                if d.get('revision')!=self.revision: raise ValueError('标定草稿已变化，请刷新后重试')
                lo,hi=self.profile['lim'][str(sid)]
                if op=='step':
                    degrees=d['degrees']
                    if type(degrees) not in (int,float) or not math.isfinite(degrees) or not .1<=abs(degrees)<=45:
                        raise ValueError('步长需在0.1～45度之间')
                    current=int.from_bytes(self.bus.read(sid,56,2),'little')
                    if not 0<=current<=4095: raise IOError('当前位置无效')
                    target=current+round(self.profile['sign'][str(sid)]*degrees/COUNT_DEG)
                    # If already outside the saved window, allow only recovery toward it.
                    if current<lo:
                        if target<=current: raise ValueError('已低于下限，只允许朝范围内移动')
                        target=min(target,hi)
                    elif current>hi:
                        if target>=current: raise ValueError('已高于上限，只允许朝范围内移动')
                        target=max(target,lo)
                    else:target=min(hi,max(lo,target))
                    if target==current:
                        if sid in self.profile.get('limits_pending',[]): raise ValueError('已到编码器单圈边界，不能继续此方向；这不是机械限位，不能按整圈绕回')
                        raise ValueError('已到所设限位，不能继续此方向')
                    targets=[target];labels=['正在步进，完成后保持'];kind='step'
                else:
                    if sid in self.profile.get('limits_pending',[]): raise ValueError('该关节尚未确认机械限位，不能自动测试；请先手动设置上下限并确认')
                    targets=[hi,lo,round((lo+hi)/2)]
                    labels=['正在缓慢前往计数上限','正在缓慢前往计数下限','正在回到上下限中间'];kind='limits'
                result=self.motion.start(sid,targets,d.get('speed',5),kind,labels)
                result['message']='ID%s %s'%(sid,result['message'])
                return result
            if op=='preview':
                if self.motion.job: raise ValueError('请先停止实物运动，再修改标定')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新页面重试')
                self.profile=copy.deepcopy(validate_profile(d['profile'])); self.revision+=1
                return {'message':'编辑已更新到草稿；尚未保存'}
            if op=='discard':
                if self.motion.job: raise ValueError('请先停止实物运动，再撤销草稿')
                self.profile=copy.deepcopy(self.saved); self.revision+=1; self.samples={}
                return {'message':'已恢复已保存参数'}
            if op=='save_one':
                if self.motion.job: raise ValueError('请先停止实物运动，再保存')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新页面重试')
                sid=d.get('id')
                if type(sid) is not int or sid not in IDS: raise ValueError('无效舵机ID')
                key=str(sid);saved=copy.deepcopy(self.saved);draft=copy.deepcopy(self.profile)
                changed_pose=any(saved[field][key]!=draft[field][key] for field in ('zero','sign'))
                for field in ('zero','sign','lim'):saved[field][key]=copy.deepcopy(draft[field][key])
                if 'limits_pending' in saved or 'limits_pending' in draft:
                    pending=set(saved.get('limits_pending',[]))
                    if sid in draft.get('limits_pending',[]):pending.add(sid)
                    else:pending.discard(sid)
                    saved['limits_pending']=[i for i in IDS if i in pending]
                if changed_pose:
                    saved.pop('reference_pose',None);draft.pop('reference_pose',None)
                saved['saved_at']=time.strftime('%Y-%m-%d %H:%M:%S');draft['saved_at']=saved['saved_at']
                validate_profile(saved);validate_profile(draft)
                atomic_save(self.args.profile,saved)
                self.saved=saved;self.profile=draft;self.revision+=1
                return {'message':'ID%s 的HOME、方向和活动范围已保存并备份；其他关节和IMU草稿未保存，力矩状态不变'%sid}
            if op=='save':
                if self.motion.job: raise ValueError('请先停止实物运动，再保存')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新页面重试')
                validate_profile(self.profile)
                saved=copy.deepcopy(self.profile); saved['saved_at']=time.strftime('%Y-%m-%d %H:%M:%S')
                atomic_save(self.args.profile,saved); self.saved=saved; self.profile=copy.deepcopy(saved); self.revision+=1
                runtime=runtime_support(self.args.robotd)
                if runtime['supported']:
                    return {'message':'标定已保存并留备份；正式程序（robotd %s）启动时读取，点“退出标定并应用到正式控制”后生效'%runtime['sha256'][:8]}
                return {'message':'已保存，但当前正式程序（robotd %s）不读网页标定，保存不会影响网页控制'%(runtime['sha256'] or '未知')[:8],'warning':True}
            if op in ('apply_exit','prepare_exit'):
                if d.get('confirm') is not True: raise ValueError('需要确认机器人已躺好或扶住')
                if self.motion.job: raise ValueError('请先停止实物运动，再退出标定')
                if d.get('revision')!=self.revision: raise ValueError('草稿已变化，请刷新页面重试')
                if self.profile!=self.saved: raise ValueError('有未保存草稿：请先保存或撤销，再退出标定')
                for sid in IDS:
                    if self.bus.read(sid,40,1)[0]!=0: raise ValueError('退出前请先全部松力矩')
                if op=='prepare_exit':
                    self.leaving=True
                    return {'message':'草稿、运动和力矩已核对，准备退出标定'}
                runtime=runtime_support(self.args.robotd)
                cmd=apply_command()
                if self.args.mock or self.args.dry_run_apply:
                    print('apply_exit dry-run:',json.dumps(cmd,ensure_ascii=False),flush=True)
                    return {'message':'离线演示：未执行，将运行 '+' '.join(cmd),'command':cmd,'runtime_support':runtime}
                r=subprocess.run(cmd,capture_output=True,text=True,timeout=5)
                if r.returncode: raise IOError('退出标定命令提交失败：'+(r.stderr or r.stdout).strip()[-300:])
                return {'message':'已提交：停止标定 → 启动robotd → 启动网页控制。robotd启动后为xgo_off，不会上力；本页随后断开，请改用网页控制（9527）。'+
                        ('' if runtime['supported'] else '注意：当前robotd不读网页标定。'),'command':cmd,'runtime_support':runtime,
                        'warning':not runtime['supported']}
            if op in ('pose','bias'):
                if self.motion.job: raise ValueError('请先停止实物运动，再采集IMU')
                if not self.imu or time.monotonic()-self.imu['at']>.3: raise ValueError('IMU没有新鲜反馈，不能采集')
                h=[v for v in self.history if time.monotonic()-v['at']<2]
                if len(h)<20 or h[-1]['at']-h[0]['at']<1: raise ValueError('请保持静止至少2秒再采集')
                avg=[sum(v['gyro'][i] for v in h)/len(h) for i in range(3)]
                spread=max(math.sqrt(sum((v['gyro'][i]-avg[i])**2 for v in h)/len(h)) for i in range(3))
                if spread>.025 or max(abs(x) for x in avg)>.15: raise ValueError('机器人仍在移动，请保持静止')
                gs=[gravity(v['quat']) for v in h]; g=norm([sum(v[i] for v in gs) for i in range(3)])
                if max(math.acos(max(-1,min(1,dot(g,v)))) for v in gs)>.04: raise ValueError('姿态未保持稳定，请静止2秒')
                if op=='bias':
                    self.profile['imu']['gyro_bias']=avg; self.revision+=1
                    return {'message':'静止偏置已采集到草稿，请检查后保存','bias':avg}
                name=d['pose']
                if name not in POSES: raise ValueError('未知姿态')
                self.samples[name]=g
                return {'message':'姿态采集完成','samples':self.samples}
            if op=='solve':
                if self.motion.job: raise ValueError('请先停止实物运动，再计算安装方向')
                q,residual=solve_mount(self.samples)
                self.profile['imu']['mount']=q; self.revision+=1
                return {'message':'安装方向已算出并预览，请倾斜核对后保存','residual_deg':residual,'mount':q}
            raise ValueError('未知操作')

    def close(self):
        self.stop.set(); self.thread.join(timeout=3)
        with self.lock:
            self.motion.cancel('服务退出，已取消运动',release=True)
            for sid in IDS:
                try: self.bus.torque(sid,False)
                except Exception as e: print('shutdown torque-off:',sid,str(e),flush=True)
            failures=self.bus.restore_speeds()
            self.bus.close()
            if self.reader: self.reader.stop()
            if failures:raise IOError('标定退出时速度恢复未确认：'+str(failures))

class Handler(SimpleHTTPRequestHandler):
    def __init__(self,*args,**kw): super().__init__(*args,directory=str(ROOT/'static'),**kw)
    def log_message(self,fmt,*args): pass
    def end_headers(self):
        self.send_header('Cache-Control','no-store'); self.send_header('X-Content-Type-Options','nosniff')
        super().end_headers()
    def reply(self,data,status=200):
        b=json.dumps(data,ensure_ascii=False,allow_nan=False).encode()
        self.send_response(status); self.send_header('Content-Type','application/json; charset=utf-8')
        self.send_header('Content-Length',str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        path=urlsplit(self.path).path
        if path=='/api/state': self.reply(self.server.app.state())
        elif path in ('/','/index.html'):
            data=(ROOT/'static/index.html').read_bytes().replace(b'__SETUP_PORT__',str(int(os.environ.get('EASYDUCK_SETUP_PORT','80'))).encode())
            self.send_response(200); self.send_header('Content-Type','text/html; charset=utf-8')
            self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
        else: super().do_GET()
    def do_POST(self):
        if self.path!='/api/command': self.reply({'error':'不存在'},404); return
        # JSON + same-origin only: avoid a foreign webpage issuing robot commands.
        origin=self.headers.get('Origin')
        if (origin and origin!='http://'+self.headers.get('Host','')) or not self.headers.get('Content-Type','').startswith('application/json'):
            self.reply({'error':'请求来源不匹配'},403); return
        try:
            n=int(self.headers.get('Content-Length','0'))
            if not 0<n<32768: raise ValueError('请求大小无效')
            self.reply(self.server.app.command(json.loads(self.rfile.read(n))))
        except (ValueError,KeyError,TypeError,IOError) as e: self.reply({'error':str(e)},400)

def main():
    p=argparse.ArgumentParser(); p.add_argument('--port',default='/dev/ttyS3'); p.add_argument('--http-port',type=int,default=8766)
    p.add_argument('--bind',default='0.0.0.0'); p.add_argument('--baseline',default='/etc/robot/zero_pos.json')
    p.add_argument('--profile',default='/etc/robot/visual_calibration.json')
    p.add_argument('--runtime-marker',default='/etc/robot/visual-calibration-runtime-v1'); p.add_argument('--mock',action='store_true')
    p.add_argument('--mock-servo-id',type=int,choices=range(254),help='single-servo offline preview (requires --mock)')
    p.add_argument('--id-journal',default='/var/lib/easyduck/servo-id-changes.jsonl')
    p.add_argument('--robotd',default='/opt/robot/daemon/bin/robotd')
    p.add_argument('--dry-run-apply',action='store_true',help='print the apply/exit systemd command instead of running it')
    args=p.parse_args();
    if args.mock_servo_id is not None and not args.mock:p.error('--mock-servo-id requires --mock')
    app=App(args); server=ThreadingHTTPServer((args.bind,args.http_port),Handler); server.app=app
    def stop(*_): threading.Thread(target=server.shutdown,daemon=True).start()
    signal.signal(signal.SIGTERM,stop); signal.signal(signal.SIGINT,stop)
    print('calibration listening',args.http_port,'mock=',args.mock,flush=True)
    try: server.serve_forever()
    finally: server.server_close(); app.close()
if __name__=='__main__': main()
