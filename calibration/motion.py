"""One selected servo at a time; called under the application's bus lock."""
import math
import time

COUNT_DEG=360/4096
SPEED_UNIT_DEG_S=.732*360/60

class MotionController:
    def __init__(self,bus):
        self.bus=bus; self.job=None; self.status={'active':False,'message':'没有实物运动任务'}

    def start(self,sid,targets,speed,kind,labels,now=None):
        if self.job: raise ValueError('已有运动任务，请等完成或点击停止')
        if type(speed) not in (int,float) or not math.isfinite(speed) or not SPEED_UNIT_DEG_S<=speed<=20:
            raise ValueError('速度需在4.4～20度/秒之间')
        if not targets or any(type(p) is not int or not 0<=p<=4095 for p in targets): raise ValueError('目标计数无效')
        if self.bus.read(sid,33,1) not in (b'\0',b'\4'): raise ValueError('舵机不是位置模式')
        current=int.from_bytes(self.bus.read(sid,56,2),'little')
        if not 0<=current<=4095: raise IOError('当前位置无效')
        state=self.bus.read(sid,40,1)[0]
        if state not in (0,1): raise IOError('力矩状态未知')
        units=max(1,int(speed/SPEED_UNIT_DEG_S))
        try:
            # Configure the speed before arming; torque-on captures the current goal.
            if state:self.bus.goal(sid,current)
            self.bus.set_speed(sid,units)
            if not state: self.bus.torque(sid,True)
            current=int.from_bytes(self.bus.read(sid,56,2),'little')
            if not 0<=current<=4095: raise IOError('上力后位置无效')
        except Exception:
            try:self.bus.torque(sid,False)
            except Exception:pass
            raise
        now=time.monotonic() if now is None else now
        rate=units*SPEED_UNIT_DEG_S/COUNT_DEG
        self.job={'id':sid,'targets':list(targets),'stage':0,'position':float(current),'sent':current,
                  'rate':rate,'last':now,'deadline':now+abs(targets[0]-current)/rate+5,
                  'kind':kind,'labels':labels,'tolerance':min(4,abs(targets[0]-current)//4),'settled':0,'previous_feedback':current,'results':[]}
        self.status={'active':True,'id':sid,'kind':kind,'stage':0,'target':targets[0],
                     'speed_deg_s':round(units*SPEED_UNIT_DEG_S,3),'message':labels[0]}
        return self.status.copy()

    def tick(self,feedback,now=None):
        if not self.job:return
        j=self.job; now=time.monotonic() if now is None else now
        try:
            r=feedback.get(j['id'])
            if not r or r.get('count') is None or now-r.get('at',0)>.4: raise IOError('运动舵机反馈丢失')
            if r.get('torque') is not True: raise IOError('运动舵机已松力矩，任务取消')
            if now>j['deadline']: raise IOError('未在预计时间到达目标，已取消任务并松力矩')
            target=j['targets'][j['stage']]
            dt=min(.1,max(0,now-j['last']));j['last']=now
            remaining=target-j['position']; distance=j['rate']*dt
            j['position']=float(target) if abs(remaining)<=distance else j['position']+math.copysign(distance,remaining)
            count=round(j['position'])
            if count!=j['sent']:
                self.bus.goal(j['id'],count);j['sent']=count
            # A loaded joint may hold with a static position error. Record it; do
            # not turn normal gravity deflection into an automatic torque cut.
            stable=abs(r['count']-j['previous_feedback'])<=2
            j['previous_feedback']=r['count']
            reached=abs(r['count']-target)<=j['tolerance']
            settled=count==target and (reached or stable)
            j['settled']=j['settled']+1 if settled else 0
            if j['settled']>=3:
                j['results'].append({'target':target,'actual':r['count'],'error_counts':r['count']-target})
                if j['stage']+1==len(j['targets']):
                    message={'limits':'上下限测试流程完成，保持中间目标','home':'回HOME目标已下发，保持中','step':'步进目标已下发，保持中'}[j['kind']]
                    warning=not reached
                    if warning:message='目标已发送，但实际未到位；保持当前目标，请检查负载、供电或关节阻挡'
                    self.status.update(active=False,message=message,warning=warning,reached=reached,actual=r['count'],results=j['results'],feedback_error_counts=r['count']-target)
                    self.job=None
                else:
                    if not reached:
                        self.status.update(active=False,warning=True,reached=False,actual=r['count'],message='本段未到目标，未继续下一段；保持当前目标',results=j['results'],feedback_error_counts=r['count']-target)
                        self.job=None
                        return
                    j['stage']+=1;j['settled']=0
                    target=j['targets'][j['stage']];j['deadline']=now+abs(target-j['position'])/j['rate']+5
                    self.status.update(stage=j['stage'],target=target,message=j['labels'][j['stage']])
        except Exception as e:self.cancel(str(e),release=True,error=True)

    def cancel(self,message='已停止并保持当前位置',release=False,error=False):
        if not self.job:return {'active':False,'message':'当前没有实物运动任务'}
        sid=self.job['id'];self.job=None
        try:
            if release:self.bus.torque(sid,False)
            else:
                p=int.from_bytes(self.bus.read(sid,56,2),'little')
                if not 0<=p<=4095:raise IOError('停止位置无效')
                if self.bus.read(sid,40,1)[0]!=1:raise IOError('已松力矩，不再写目标')
                self.bus.goal(sid,p)
            self.status.update(active=False,message=message,error=error)
        except Exception as e:
            try:self.bus.torque(sid,False)
            except Exception:pass
            self.status.update(active=False,message='停止保持未确认，已尝试松力矩：'+str(e),error=True)
        return self.status.copy()


class MotionCoordinator:
    """One individual task or one coordinated HOME task; never both."""
    def __init__(self,bus):
        self.bus=bus;self.single=MotionController(bus);self.members=[];self.mode='single';self.group_status={}

    @property
    def job(self):
        if self.mode=='group':
            return {'id':None,'ids':[sid for sid,_ in self.members]} if any(m.job for _,m in self.members) else None
        return self.single.job

    @property
    def status(self):return self.group_status if self.mode=='group' else self.single.status

    def owns(self,sid):
        job=self.job
        return bool(job and (job.get('id')==sid or sid in job.get('ids',[])))

    def start(self,*args,**kwargs):
        if self.job:raise ValueError('已有运动任务，请先停止')
        self.mode='single';self.members=[]
        return self.single.start(*args,**kwargs)

    def start_home(self,targets,speed=5):
        if self.job:raise ValueError('已有运动任务，请先停止')
        if not targets or any(type(v) is not int or not 0<=v<=4095 for v in targets.values()):raise ValueError('HOME计数无效')
        self.mode='group';self.members=[]
        try:
            for sid,target in targets.items():
                controller=MotionController(self.bus)
                self.members.append((sid,controller))
                controller.start(sid,[target],speed,'home',['正在缓慢回HOME'])
        except Exception:
            self.cancel('回HOME准备失败，已取消',release=True)
            raise
        now=time.monotonic()
        for _,m in self.members:
            m.job['last']=now;m.job['deadline']=now+abs(m.job['targets'][0]-m.job['position'])/m.job['rate']+5
        self.group_status={'active':True,'kind':'home','ids':list(targets),'message':'15个舵机正在缓慢回HOME',
                           'speed_deg_s':self.members[0][1].status['speed_deg_s']}
        return self.group_status.copy()

    def tick(self,feedback,now=None,only=None):
        if self.mode!='group':self.single.tick(feedback,now);return
        if not self.job:return
        for sid,m in self.members:
            if only is not None and sid!=only:continue
            m.tick(feedback,now)
            if m.status.get('error'):
                self.cancel('ID%s 回HOME失败：%s'%(sid,m.status['message']),release=True,error=True)
                return
        done=sum(m.job is None for _,m in self.members)
        self.group_status.update(active=done<len(self.members),message='15个舵机回HOME目标已下发，保持中' if done==len(self.members) else '正在缓慢回HOME，进度 %s/15'%done)
        if done==len(self.members):
            self.group_status['results']={sid:m.status.get('results',[]) for sid,m in self.members}
            missed=[sid for sid,m in self.members if m.status.get('warning')]
            if missed:self.group_status.update(warning=True,reached=False,message='HOME目标已下发，但以下关节未到位：'+str(missed))

    def cancel(self,message='已停止并保持当前位置',release=False,error=False):
        if self.mode!='group':return self.single.cancel(message,release,error)
        if not self.job and not release:return {'active':False,'message':'当前没有实物运动任务'}
        failures=[]
        for sid,m in self.members:
            result=m.cancel(message,release,error)
            if result.get('error') and not release:failures.append(sid)
            if release:
                # Include children that had already arrived or failed during preparation.
                try:self.bus.torque(sid,False)
                except Exception:failures.append(sid)
        self.group_status.update(active=False,message=message+('；未确认项：'+str(sorted(set(failures))) if failures else ''),error=error or bool(failures))
        return self.group_status.copy()
