"""Explicit IMU selection in the shared robotd TOML; never writes calibration."""
import json,os,re,stat,tempfile,time,tomllib
from pathlib import Path
from imu import configuration,validate_config

KEYS=('source','model','i2c_dev','i2c_addr','fusion')
def selected(current,selection):
 if not isinstance(selection,dict) or set(selection)!={'model','i2c_dev','i2c_addr','fusion'}:raise ValueError('请完整选择型号、总线、地址和融合方式')
 if not isinstance(selection['i2c_dev'],str) or not re.fullmatch(r'/dev/i2c-[0-9]+',selection['i2c_dev']):raise ValueError('I2C总线格式无效')
 cfg=dict(current);cfg.update(selection,source='i2c');validate_config(cfg);return cfg

def prepare(path,selection):
 path=Path(path)
 if path.is_symlink():raise ValueError('硬件配置是符号链接，请先人工确认其目标')
 if not isinstance(selection,dict) or set(selection)!={'model','i2c_dev','i2c_addr','fusion'}:raise ValueError('请完整选择型号、总线、地址和融合方式')
 if not isinstance(selection['i2c_dev'],str) or not re.fullmatch(r'/dev/i2c-[0-9]+',selection['i2c_dev']):raise ValueError('I2C总线格式无效')
 original=path.read_bytes();cfg=selected(configuration(path),selection)
 text=original.decode('utf-8');lines=text.splitlines(keepends=True)
 headers=[i for i,line in enumerate(lines) if re.match(r'^\s*\[',line)]
 starts=[i for i in headers if re.fullmatch(r'\s*\[imu\]\s*(?:#.*)?\n?',lines[i])]
 if len(starts)>1:raise ValueError('硬件配置中存在重复的IMU段落')
 if not starts:
  if 'imu' in tomllib.loads(text):raise ValueError('内联IMU配置不能在此修改')
  lines.append('\n[imu]\n');start=len(lines)-1;end=len(lines)
 else:start=starts[0];end=next((i for i in headers if i>start),len(lines))
 if not lines[start].endswith('\n'):lines[start]+='\n'
 block=lines[start+1:end]
 if block and not block[-1].endswith('\n'):block[-1]+='\n'
 for key in KEYS:
  value=json.dumps(cfg[key],ensure_ascii=True);replacement=key+' = '+value+'\n'
  matches=[i for i,line in enumerate(block) if re.match(r'^\s*'+key+r'\s*=',line)]
  if len(matches)>1:raise ValueError('IMU配置字段重复：'+key)
  if matches:block[matches[0]]=replacement
  else:block.append(replacement)
 updated=''.join(lines[:start+1]+block+lines[end:])
 parsed=tomllib.loads(updated)
 for key in KEYS:
  if parsed['imu'][key]!=cfg[key]:raise ValueError('硬件配置写入校验失败')
 return original,updated,cfg

def commit(path,original,updated):
 path=Path(path)
 if path.read_bytes()!=original:raise ValueError('硬件配置已被其他操作修改，请刷新后重试')
 backup_dir=path.parent/'hardware-config-backups';backup_dir.mkdir(mode=0o700,exist_ok=True)
 backup=backup_dir/('robotd-'+str(time.time_ns())+'.toml')
 fd=os.open(backup,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'wb') as f:f.write(original);f.flush();os.fsync(f.fileno())
 fd,temp=tempfile.mkstemp(prefix='.robotd-imu-',dir=path.parent)
 try:
  with os.fdopen(fd,'w',encoding='utf-8',newline='\n') as f:f.write(updated);f.flush();os.fsync(f.fileno())
  os.chmod(temp,stat.S_IMODE(path.stat().st_mode));os.replace(temp,path)
 finally:
  if os.path.exists(temp):os.unlink(temp)
 return backup


FIXED_BUS='/dev/i2c-2'

def probe_lsm(address):
 from imu.transport import I2c
 bus=I2c(FIXED_BUS,address)
 try:return bus.reg(0x0f)==bytes([0x70])
 finally:bus.close()

def probe_bno(address):
 # Query identity only: no reset, feature changes or calibration writes.
 from imu.bno085 import BnoDriver
 from imu.transport import I2c
 driver=BnoDriver.__new__(BnoDriver)
 driver.bus=I2c(FIXED_BUS,address);driver.tx=[0]*6;driver.rx=[None]*6
 try:
  driver.send(2,[0xf9,0]);deadline=time.monotonic()+2
  while time.monotonic()<deadline:
   packet=driver.packet()
   if packet and packet[0]==2 and packet[1][:1]==bytes([0xf8]):
    data=packet[1]
    return len(data)>=16 and len(data)%16==0 and all(data[i]==0xf8 for i in range(0,len(data),16))
   time.sleep(.02)
  return False
 finally:driver.close()

def detect():
 found=[]
 for model,addresses,fusion,probe in [('lsm6dsv16x',(0x6a,0x6b),'sflp',probe_lsm),('bno085',(0x4a,0x4b),'game_rv',probe_bno)]:
  for address in addresses:
   try:identified=probe(address)
   except (OSError,ValueError):identified=False
   if identified:found.append(dict(model=model,i2c_dev=FIXED_BUS,i2c_addr=address,fusion=fusion))
 if not found:raise ValueError('未找到支持的 IMU，请检查传感器接线和供电后重试')
 if len(found)>1:raise ValueError('检测到多个 IMU，请只连接一个传感器后重试')
 return found[0]
