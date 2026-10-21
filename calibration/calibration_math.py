"""Shared conventions: wxyz quaternions; mount rotates sensor vectors into body."""
import math

IDS = [20,21,22,23,24,30,31,32,33,34,10,11,12,13,14]
NAMES = ['left_hip_yaw','left_hip_roll','left_hip_pitch','left_knee','left_ankle',
         'neck_pitch','head_pitch','head_yaw','head_roll','mouth',
         'right_hip_yaw','right_hip_roll','right_hip_pitch','right_knee','right_ankle']
LABELS = ['左髋转向','左髋侧摆','左髋前后','左膝','左踝','颈俯仰','头俯仰','头转向','头侧摆','嘴部',
          '右髋转向','右髋侧摆','右髋前后','右膝','右踝']
HOME = [0,-.0873,-.4188790204786391,-.0049,.4188790204786391,.3491,.3491,0,0,0,
        0,.0873,.4188790204786391,.0049,-.4188790204786391]
DEFAULT_MOUNT = [math.sqrt(.5),0,-math.sqrt(.5),0]
POSES = {'level':[0,0,-1], 'front_down':[1,0,0], 'left_down':[0,1,0],
         'front_up':[-1,0,0], 'right_down':[0,-1,0]}

def norm(v):
    n = math.sqrt(sum(x*x for x in v))
    if not math.isfinite(n) or n < 1e-9: raise ValueError('无效向量')
    return [x/n for x in v]

def mul(a,b):
    w,x,y,z=a; v,i,j,k=b
    return [w*v-x*i-y*j-z*k,w*i+x*v+y*k-z*j,w*j-x*k+y*v+z*i,w*k+x*j-y*i+z*v]

def inv(q): return [q[0],-q[1],-q[2],-q[3]]
def rotate(q,v): return mul(mul(q,[0]+list(v)),inv(q))[1:]
def gravity(q): return rotate(inv(q),[0,0,-1])
def angle(home,count,zero,sign): return home+sign*(count-zero)*2*math.pi/4096
def cross(a,b): return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def dot(a,b): return sum(x*y for x,y in zip(a,b))

def solve_mount(samples):
    # TRIAD basis from two independent poses, then independently score every pose.
    if 'level' not in samples or not any(p in samples for p in ('front_down','left_down')):
        raise ValueError('先采集站直，以及前倾90°或左倾90°；建议三种都采集')
    second = 'front_down' if 'front_down' in samples else 'left_down'
    a=norm(samples['level']); b=norm(samples[second])
    if abs(dot(a,b)) > .35: raise ValueError('两次姿态没有接近垂直，请重新摆放采集')
    def basis(u,v):
        e1=norm(u); e2=norm([v[i]-dot(e1,v)*e1[i] for i in range(3)])
        return [e1,e2,cross(e1,e2)]
    s=basis(a,b); t=basis(POSES['level'],POSES[second])
    m=[[sum(t[k][i]*s[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
    # Stable matrix to quaternion conversion including half turns.
    tr=sum(m[i][i] for i in range(3))
    if tr>0:
        r=math.sqrt(1+tr)*2
        q=[r/4,(m[2][1]-m[1][2])/r,(m[0][2]-m[2][0])/r,(m[1][0]-m[0][1])/r]
    else:
        i=max(range(3),key=lambda k:m[k][k]); j=(i+1)%3; k=(i+2)%3
        r=math.sqrt(1+m[i][i]-m[j][j]-m[k][k])*2
        q=[(m[k][j]-m[j][k])/r,0,0,0]
        q[i+1]=r/4; q[j+1]=(m[j][i]+m[i][j])/r; q[k+1]=(m[k][i]+m[i][k])/r
    q=norm(q)
    residual={p:math.degrees(math.acos(max(-1,min(1,dot(norm(rotate(q,v)),POSES[p]))))) for p,v in samples.items()}
    if max(residual.values())>8: raise ValueError('姿态之间不一致（偏差超过8°），请重新采集')
    return q,residual

def validate_profile(d):
    for key in ('zero','sign','lim'):
        if set(d.get(key,{})) != {str(i) for i in IDS}: raise ValueError('标定必须包含15个舵机')
    for sid in IDS:
        k=str(sid); z=d['zero'][k]; s=d['sign'][k]; lim=d['lim'][k]
        if type(z) is not int or type(s) is not int or s not in (-1,1) or len(lim)!=2 or any(type(x) is not int for x in lim):
            raise ValueError('零点/范围必须是整数，方向只能为±1')
        if not 0<=lim[0]<=z<=lim[1]<=4095 or lim[0]==lim[1]: raise ValueError('零点必须在有效范围内（0～4095）')
    imu=d['imu']; mount=imu['mount']; bias=imu['gyro_bias']
    if len(mount)!=4 or len(bias)!=3 or not all(type(x) in (int,float) and math.isfinite(x) for x in mount+bias):
        raise ValueError('IMU参数无效')
    if abs(sum(x*x for x in mount)-1)>.001 or max(abs(x) for x in bias)>.2: raise ValueError('IMU旋转或静止偏置无效')
    return d
