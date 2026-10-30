const $=id=>document.getElementById(id);
// Theme state is shared with connection setup; it never sends control requests.
function setTheme(theme){
 const selected=theme==='day'?'day':'night';
 document.documentElement.dataset.theme=selected;
 $('theme-button').textContent=selected==='day'?'☾ 暗色':'☼ 亮色';
 $('theme-button').setAttribute('aria-pressed',String(selected==='day'));
 try{localStorage.setItem('easyduck-theme',selected);}catch(_){}
}
try{setTheme(localStorage.getItem('easyduck-theme')||'night');}catch(_){setTheme('night');}
$('theme-button').onclick=()=>setTheme(document.documentElement.dataset.theme==='day'?'night':'day');
let statusBusy=false, commandBusy=false, commandPending=false;
const controlId=globalThis.crypto?.randomUUID?.()||`${Date.now()}-${Math.random().toString(16).slice(2)}`;
let controlGeneration=null, webAllowed=false;
function resetInput(){keys.clear();cancelCall();for(const i of [0,3])$('head'+i).value=0;zero();}
$('use-web').onclick=async()=>{try{resetInput();await post('command',{twist:[0,0,0],head:[0,0,0,0],mouth:0});await post('control',{preference:'web'});}catch(e){showError(e);}};
$('use-pad').onclick=async()=>{try{resetInput();await post('control',{preference:'auto'});}catch(e){showError(e);}};
function releasePage(){return fetch('/api/release',{method:'POST',keepalive:true,headers:{'Content-Type':'application/json','X-Control-Id':controlId},body:'{}'}).catch(()=>{});}
async function post(path, body){const r=await fetch('/api/'+path,{method:'POST',headers:{'Content-Type':'application/json','X-Control-Id':controlId},body:JSON.stringify(body)});const x=await r.json();if(!r.ok)throw new Error(x.detail||'请求未完成，请重试');return x;}
let noticeUntil=0,skillBusy=false;
function notice(text,error=false){noticeUntil=Date.now()+4000;$('message').textContent=text;$('message').classList.toggle('error',error);}
function showError(e){notice(e.message,true);}
async function setMode(mode){if(mode!=='off'&&!webAllowed)return;try{await post('mode',{mode});notice(mode==='off'?'已发送断力请求':mode==='hold'?'已发送默认站姿请求':'已发送开启行走请求');}catch(e){showError(e);}}
document.querySelectorAll('[data-mode]').forEach(b=>b.onclick=()=>setMode(b.dataset.mode));
document.querySelectorAll('[data-skill]').forEach(button=>button.onclick=async()=>{
 if(!webAllowed||skillBusy||button.disabled)return;
 skillBusy=true;
 const label=button.querySelector('b').textContent;button.disabled=true;
 try{zero();await post('skill',{skill:button.dataset.skill});notice('已发送'+label+'请求');}catch(e){showError(e);}finally{skillBusy=false;}
});
$('head-zero').onclick=()=>{resetSticks[1]();for(const i of [0,3])$('head'+i).value=0;sendCommand();};
for(const input of document.querySelectorAll('#head-controls input'))input.oninput=()=>sendCommand();
$('speed').oninput=()=>{$('speed-value').textContent=$('speed').value+'%';updateMotion();sendCommand();};
$('stop').onclick=()=>{zero();setMode('off')};
const keys=new Set(), sticks={left:{x:0,y:0},right:{x:0,y:0}}, resetSticks=[];
let motion=[0,0,0];
const deadzone=v=>Math.abs(v)<.1?0:v;
function updateMotion(){
  const key=(a,b)=>keys.has(a)-keys.has(b);
  motion=[keys.has('w')||keys.has('s')?key('w','s')*.2:deadzone(sticks.left.y)*.4,
    0, keys.has('a')||keys.has('d')?key('a','d')*.5:deadzone(-sticks.left.x)];
  motion=motion.map(value=>value*Number($('speed').value)/100);
}
async function sendCommand(){
  if(pageClosing)return;
  if(commandBusy){commandPending=true;return;}
  commandBusy=true;commandPending=false;
  try{await post('command',{twist:[...motion],head:[+$('head0').value,-deadzone(sticks.right.y),-deadzone(sticks.right.x),+$('head3').value],mouth:callLevel*30,vocal:callHeld?callLevel:0});}
  catch(e){showError(e);}
  finally{commandBusy=false;if(commandPending)sendCommand();}
}
function joystick(id,side){
  const zone=$(id),knob=zone.firstElementChild;
  let pointer=null;
  function reset(){
    const old=pointer;pointer=null;
    sticks[side]={x:0,y:0};knob.style.transform='translate(0px,0px)';
    if(old!==null&&zone.hasPointerCapture(old))zone.releasePointerCapture(old);
  }
  resetSticks.push(reset);
  function move(e){
    if(!webAllowed)return;
    const b=zone.getBoundingClientRect(),radius=(zone.clientWidth-knob.offsetWidth)/2;
    let dx=e.clientX-b.left-b.width/2,dy=e.clientY-b.top-b.height/2;
    const distance=Math.hypot(dx,dy);
    if(distance>radius){dx*=radius/distance;dy*=radius/distance;}
    knob.style.transform=`translate(${dx}px,${dy}px)`;
    sticks[side]={x:dx/radius,y:-dy/radius};updateMotion();
  }
  zone.onpointerdown=e=>{if(!webAllowed||pointer!==null||e.button!==0)return;e.preventDefault();pointer=e.pointerId;zone.setPointerCapture(pointer);move(e);sendCommand();};
  zone.onpointermove=e=>{if(e.pointerId===pointer)move(e);};
  const release=e=>{if(e.pointerId!==pointer)return;reset();updateMotion();sendCommand();};
  zone.onpointerup=zone.onpointercancel=zone.onlostpointercapture=release;
}
joystick('joy-left','left');joystick('joy-right','right');
function zero(){cancelCall();keys.clear();resetSticks.forEach(reset=>reset());updateMotion();sendCommand();}
let callPointer=null,callHeld=false,callLevel=0,callTick=performance.now();
function renderCall(){
 $('call').setAttribute('aria-pressed',String(callHeld));
 $('call').style.setProperty('--call-level',`${Math.round(callLevel*100)}%`);
 $('call-level').textContent=callLevel>0?`${callHeld?'鸣叫':'回落中'} ${Math.round(callLevel*100)}%`:'按得越久，叫声越响';
}
function releaseCall(){
 const id=callPointer;callPointer=null;callHeld=false;callLevel=0;
 if(id!==null&&$('call').hasPointerCapture(id))$('call').releasePointerCapture(id);
 renderCall();sendCommand();
}
function cancelCall(){callHeld=false;callLevel=0;const id=callPointer;callPointer=null;
 if(id!==null&&$('call').hasPointerCapture(id))$('call').releasePointerCapture(id);renderCall();}
$('call').onpointerdown=e=>{if(!webAllowed||callPointer!==null||e.button!==0)return;e.preventDefault();callPointer=e.pointerId;callHeld=true;callTick=performance.now();$('call').setPointerCapture(e.pointerId);renderCall();sendCommand();};
$('call').onpointerup=$('call').onpointercancel=$('call').onlostpointercapture=e=>{if(e.pointerId===callPointer)releaseCall();};
$('call').onkeydown=e=>{if((e.key==='Enter'||e.key===' ')&&webAllowed&&!e.repeat){e.preventDefault();callHeld=true;callTick=performance.now();renderCall();}};
$('call').onkeyup=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();releaseCall();}};
$('call').onblur=()=>{if(callHeld)releaseCall();};
setInterval(()=>{const now=performance.now(),dt=Math.min((now-callTick)/1000,.1);callTick=now;
 if(!webAllowed){if(callLevel||callHeld)cancelCall();return;}
 const next=callHeld?Math.min(1,callLevel+dt/.35):Math.max(0,callLevel-dt/.45);
 if(next!==callLevel){callLevel=next;renderCall();sendCommand();}
},50);
function keyCommand(){updateMotion();sendCommand();}
window.addEventListener('keydown',e=>{if(['INPUT','TEXTAREA','SELECT','BUTTON'].includes(e.target.tagName)||e.target.isContentEditable)return;if(e.code==='Space'){e.preventDefault();$('stop').click();return;}const k=e.key.toLowerCase();if(webAllowed&&'wasd'.includes(k)&&k.length===1){e.preventDefault();keys.add(k);keyCommand();}});
window.addEventListener('keyup',e=>{const k=e.key.toLowerCase();if(keys.delete(k))keyCommand();});
window.addEventListener('blur',()=>{keys.clear();zero();});
document.addEventListener('visibilitychange',()=>{if(document.hidden)zero();});
let controlSocket=null,pageClosing=false;
function connectControl(){
  if(pageClosing)return;
  const scheme=location.protocol==='https:'?'wss:':'ws:';
  const socket=new WebSocket(`${scheme}//${location.host}/api/connection?client=${encodeURIComponent(controlId)}`);
  controlSocket=socket;
  socket.onclose=()=>{if(!pageClosing&&controlSocket===socket)setTimeout(connectControl,500);};
}
window.addEventListener('pagehide',()=>{pageClosing=true;releasePage();controlSocket?.close();});
window.addEventListener('pageshow',()=>{if(pageClosing){pageClosing=false;connectControl();}});
connectControl();
setInterval(()=>{if(!pageClosing&&!document.hidden&&!commandBusy)sendCommand();},50);
// Display only telemetry fields provided by robotd.
// Missing counters or per-servo torque remain unknown.
setInterval(async()=>{if(statusBusy||document.hidden)return;statusBusy=true;try{
const r=await fetch('/api/status',{headers:{'X-Control-Id':controlId},signal:AbortSignal.timeout(2500)});if(!r.ok)throw new Error('机器人状态读取失败，请检查连接');const s=await r.json();
const fresh=s.feedback_age_ms!=null&&s.feedback_age_ms<1500&&!s.last_error;
const control=s.control||{};
if(controlGeneration!==control.generation){resetInput();controlGeneration=control.generation;}
webAllowed=fresh&&control.owner==='web'&&control.ready&&s.this_page;
$('control-owner').textContent=control.owner==='pad'?'手柄':control.owner==='web'?'网页':'等待连接';
$('pad-connection').textContent=control.pad_connected?'已连接':'未连接';
$('control-rule').textContent=(control.preference==='web'?'已指定网页':'手柄优先')+(control.ready?'':' · 等待控制归零');
for(const b of document.querySelectorAll('[data-mode]'))if(b.dataset.mode!=='shadow')b.disabled=!webAllowed;
for(const input of document.querySelectorAll('#head-controls input'))input.disabled=!webAllowed;
const fmt=(n,d=1)=>typeof n==='number'?n.toFixed(d):'—';
const modeLabel={off:'最近请求：断力',hold:'最近请求：默认站姿',policy:'最近请求：行走',robotd:'正在查看主控状态'};
$('rx').textContent=fmt(s.feedback_hz)+' Hz';$('rl').textContent=fmt(s.loop_hz)+' Hz';
$('imu').textContent=s.imu_ok==null?'未知':s.imu_ok?'就绪':'未就绪';
$('mode').textContent=modeLabel[s.mode]||s.mode;
$('online-dot').classList.toggle('online',fresh);$('connection-label').textContent=fresh?'机器人在线':'状态未就绪';
const phase=s.state?.policy;
const driveButton=document.querySelector('[data-mode=policy]');driveButton.querySelector('b').textContent=phase==='xgo_walk'?'行走已开启':'开启行走';driveButton.disabled=!webAllowed||phase==='xgo_walk';
const phases={xgo_off:'策略已关闭',held:'保持姿态',stand:'站姿',xgo_walk:'行走模式',xgo_getup:'倒地恢复中',xgo_default_pose:'准备起身',xgo_sit:'坐姿',xgo_rise:'起立中',xgo_pick:'捡物中',xgo_roulade:'滚翻中',xgo_jump:'跳跃中',limp:'已断力'};
$('robot-phase').textContent=phases[phase]||(phase?'其他状态':'等待读取状态');
const voltage=s.health?.battery?.volts;
const percent=s.health?.battery?.percent;
$('battery').textContent=typeof percent==='number'?Math.round(percent)+'%':typeof voltage==='number'?voltage.toFixed(2)+' V':'—';$('battery').title=typeof voltage==='number'?voltage.toFixed(2)+' V':'';
$('control-guide').textContent=webAllowed?'网页已接管，可以操作':control.owner==='pad'?'手柄正在控制':'点击「用网页控制」开始';
$('use-web').disabled=webAllowed;$('use-pad').disabled=control.preference==='auto';
for(const zone of document.querySelectorAll('.joy'))zone.setAttribute('aria-disabled',String(!webAllowed));
$('call').disabled=!webAllowed;$('speed').disabled=!webAllowed;$('head-zero').disabled=!webAllowed;
if(Date.now()>noticeUntil){$('message').textContent=s.last_error||(webAllowed?'摇杆松手停止移动；动作按钮按一次执行一次。':'网页未接管，摇杆与动作按钮暂不可用。');$('message').classList.toggle('error',!!s.last_error);}
for(const button of document.querySelectorAll('[data-skill]')){button.hidden=!s.models?.[button.dataset.model];button.disabled=skillBusy||!webAllowed||phase!=='xgo_walk'&&!(button.dataset.skill==='sit_toggle'&&phase==='xgo_sit');}


const state=s.state||{},recovery=s.recovery||{};
$('recovery-phase').textContent=state.policy||'—';$('active-model').textContent=s.active_model||'—';
$('tilt-angle').textContent=recovery.angle_deg==null?'—':fmt(recovery.angle_deg)+'°';
$('recovery-timer').textContent=state.policy==='xgo_default_pose'?'原版默认姿态保持 1 秒':state.policy==='xgo_getup'?'原版倒地恢复':state.policy==='xgo_pick'?'原版 4 秒捡物周期':'—';
for(const [id,values]of [['gyro-values',s.gyro],['gravity-values',s.gravity]])$(id).textContent=values?values.map(x=>x.toFixed(3)).join(' / '):'—';
$('command-health').textContent=`请求 ${(state.move?.requested||[]).map(x=>fmt(x,3)).join(' / ')} · 应用 ${(state.move?.applied||[]).map(x=>fmt(x,3)).join(' / ')} · 限制 ${(state.move?.limited_by||[]).join(', ')||'无'}`;
if(s.angles)$('joints').innerHTML=s.motors.map((id,i)=>`<tr><td>${id}</td><td>${fmt(s.angles[i])}</td><td>${fmt(s.targets[i])}</td></tr>`).join('');
$('runtime-log').textContent=s.recent_log||'暂无日志';
const cal=s.calibration||{},calTime=t=>t==null?'—':new Date(t*1000).toLocaleString('zh-CN',{hour12:false});
const calLabel={applied:'已生效（推断）',pending_restart:'已保存，待重启robotd生效',unsupported:'当前robotd不读网页标定',unknown:'未知'}[cal.state]||'未知';
$('calibration-status').textContent=`标定状态：${calLabel} · 文件 ${cal.file_sha256||'—'} · 保存于 ${cal.saved_at||'—'}（修改 ${calTime(cal.file_mtime)}） · robotd ${(cal.robotd_sha256||'—').slice(0,8)} 启动于 ${calTime(cal.robotd_started)}${cal.error?' · '+cal.error:''}。robotd不记录已加载哪份标定，“已生效”按文件修改时间早于robotd启动时间推断。`;
$('calibration-status').className=cal.state==='applied'?'hint':'warn';
}catch(e){webAllowed=false;resetInput();$('online-dot').classList.remove('online');$('connection-label').textContent='连接中断';$('control-guide').textContent='等待机器人重新连接';for(const b of document.querySelectorAll('[data-mode],[data-skill],#call'))b.disabled=true;showError(e);}finally{statusBusy=false;}},250);

function navigationURL(target){
 const url=new URL('/navigate',location.href);url.port=document.querySelector('meta[name="setup-port"]').content;url.searchParams.set('target',target);url.searchParams.set('source','control');return url.href;
}
for(const [id,target] of [['nav-settings','setup'],['nav-calibration','calibration']]){
 const link=$(id);link.href=navigationURL(target);
 link.onclick=async event=>{event.preventDefault();if(pageClosing)return;let destination;try{destination=await window.easyduckNavigation(link.href);}catch(error){showError(error);return;}pageClosing=true;cancelCall();keys.clear();resetSticks.forEach(reset=>reset());updateMotion();await releasePage();controlSocket?.close();location.assign(destination);};
}
