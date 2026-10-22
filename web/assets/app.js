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
function resetInput(){keys.clear();zero();for(let i=0;i<4;i++)$('head'+i).value=0;$('mouth').value=0;}
$('use-web').onclick=async()=>{try{resetInput();await post('command',{twist:[0,0,0],head:[0,0,0,0],mouth:0});await post('control',{preference:'web'});}catch(e){showError(e);}};
$('use-pad').onclick=async()=>{try{resetInput();await post('control',{preference:'auto'});}catch(e){showError(e);}};
function releasePage(){return fetch('/api/release',{method:'POST',keepalive:true,headers:{'Content-Type':'application/json','X-Control-Id':controlId},body:'{}'}).catch(()=>{});}
for(let i=0;i<4;i++) $('head-controls').insertAdjacentHTML('beforeend',`<label>head ${i+1}<input id="head${i}" type="range" min="-1" max="1" value="0" step="0.02"></label>`);
$('head-controls').insertAdjacentHTML('beforeend','<label>Mouth 34 (°)<input id="mouth" type="range" min="0" max="30" value="0" step="1"></label>');
async function post(path, body){const r=await fetch('/api/'+path,{method:'POST',headers:{'Content-Type':'application/json','X-Control-Id':controlId},body:JSON.stringify(body)});const x=await r.json();if(!r.ok)throw new Error(x.detail||'request failed');return x;}
function showError(e){$('message').textContent=e.message;}
async function setMode(mode){if(mode!=='off'&&!webAllowed)return;try{await post('mode',{mode});$('message').textContent='Mode updated';}catch(e){showError(e);}}
document.querySelectorAll('[data-mode]').forEach(b=>b.onclick=()=>setMode(b.dataset.mode));
$('pick').onclick=async()=>{try{const r=await post('pick',{});$('message').textContent=`Pick started · ${r.model}`;}catch(e){showError(e);}};
$('roulade').onclick=async()=>{try{const r=await post('roulade',{});$('message').textContent=`Roulade · ${r.model}`;}catch(e){showError(e);}};
$('stop').onclick=()=>{zero();setMode('off')};
const keys=new Set(), sticks={left:{x:0,y:0},right:{x:0,y:0}}, resetSticks=[];
let motion=[0,0,0];
const deadzone=v=>Math.abs(v)<.1?0:v;
function updateMotion(){
  const key=(a,b)=>keys.has(a)-keys.has(b);
  motion=[keys.has('w')||keys.has('s')?key('w','s')*.2:deadzone(sticks.left.y)*.4,
    keys.has('a')||keys.has('d')?key('a','d')*.15:deadzone(-sticks.left.x)*.3,
    keys.has('q')||keys.has('e')?key('q','e')*.5:deadzone(-sticks.right.x)];
  ['vx','vy','wz'].forEach((id,i)=>$(id+'Value').textContent=motion[i].toFixed(2));
}
async function sendCommand(){
  if(pageClosing)return;
  if(commandBusy){commandPending=true;return;}
  commandBusy=true;commandPending=false;
  try{await post('command',{twist:[...motion],head:[0,1,2,3].map(i=>+$('head'+i).value),mouth:+$('mouth').value});}
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
  zone.onpointerdown=e=>{if(pointer!==null||e.button!==0)return;e.preventDefault();pointer=e.pointerId;zone.setPointerCapture(pointer);move(e);sendCommand();};
  zone.onpointermove=e=>{if(e.pointerId===pointer)move(e);};
  const release=e=>{if(e.pointerId!==pointer)return;reset();updateMotion();sendCommand();};
  zone.onpointerup=zone.onpointercancel=zone.onlostpointercapture=release;
}
joystick('joy-left','left');joystick('joy-right','right');
function zero(){keys.clear();resetSticks.forEach(reset=>reset());updateMotion();sendCommand();}
$('zero').onclick=zero;
['leg-alpha','head-alpha'].forEach(id=>$(id).onchange=()=>post('filter',{leg:+$('leg-alpha').value,head:+$('head-alpha').value}).catch(showError));
function keyCommand(){updateMotion();sendCommand();}
window.addEventListener('keydown',e=>{if(e.code==='Space'){e.preventDefault();$('stop').click();return;}const k=e.key.toLowerCase();if(webAllowed&&'wasdqe'.includes(k)&&k.length===1){keys.add(k);keyCommand();}});
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
const r=await fetch('/api/status',{headers:{'X-Control-Id':controlId}});if(!r.ok)throw new Error('status read failed');const s=await r.json();
const control=s.control||{};
if(controlGeneration!==control.generation){resetInput();controlGeneration=control.generation;}
webAllowed=control.owner==='web'&&control.ready&&s.this_page;
$('control-owner').textContent=control.owner==='pad'?'手柄':control.owner==='web'?'网页':'等待连接';
$('pad-connection').textContent=control.pad_connected?'已连接':'未连接';
$('control-rule').textContent=(control.preference==='web'?'已指定网页':'手柄优先')+(control.ready?'':' · 等待控制归零');
for(const b of document.querySelectorAll('[data-mode]'))if(b.dataset.mode!=='shadow')b.disabled=!webAllowed;
for(const input of document.querySelectorAll('#head-controls input'))input.disabled=!webAllowed;
const fmt=(n,d=1)=>typeof n==='number'?n.toFixed(d):'—';
const modeLabel={off:'已请求断力',hold:'已请求默认站姿',policy:'已请求策略',robotd:'仅查看现有robotd'};
$('rx').textContent=fmt(s.feedback_hz)+' Hz';$('rl').textContent=fmt(s.loop_hz)+' Hz';
$('servos').textContent='未逐个回读';$('imu').textContent=s.imu_ok==null?'未知':s.imu_ok?'online':'offline';
$('mode').textContent=modeLabel[s.mode]||s.mode;
$('message').textContent=s.last_error||`状态年龄 ${fmt(s.feedback_age_ms)} ms。请求接收不等于动作完成，扭矩状态未回读。`;
$('pick').disabled=!webAllowed||s.mode!=='policy'||s.state?.policy!=='xgo_walk'||!s.models?.ground_pick;
const state=s.state||{},recovery=s.recovery||{};
$('recovery-phase').textContent=state.policy||'—';$('active-model').textContent=s.active_model||'—';
$('tilt-angle').textContent=recovery.angle_deg==null?'—':fmt(recovery.angle_deg)+'°';
$('recovery-timer').textContent=state.policy==='xgo_default_pose'?'原版默认姿态保持 1 秒':state.policy==='xgo_getup'?'原版倒地恢复':state.policy==='xgo_pick'?'原版 4 秒捡物周期':'—';
for(const [id,values]of [['acc-values',s.acc],['gyro-values',s.gyro],['gravity-values',s.gravity]])$(id).textContent=values?values.map(x=>x.toFixed(3)).join(' / '):'—';
$('command-health').textContent=`请求 ${(state.move?.requested||[]).map(x=>fmt(x,3)).join(' / ')} · 应用 ${(state.move?.applied||[]).map(x=>fmt(x,3)).join(' / ')} · 限制 ${(state.move?.limited_by||[]).join(', ')||'无'}`;
if(s.angles)$('joints').innerHTML=s.motors.map((id,i)=>`<tr><td>${id}</td><td>${fmt(s.angles[i])}</td><td>${fmt(s.targets[i])}</td></tr>`).join('');
$('runtime-log').textContent=s.recent_log||'暂无日志';
const cal=s.calibration||{},calTime=t=>t==null?'—':new Date(t*1000).toLocaleString('zh-CN',{hour12:false});
const calLabel={applied:'已生效（推断）',pending_restart:'已保存，待重启robotd生效',unsupported:'当前robotd不读网页标定',unknown:'未知'}[cal.state]||'未知';
$('calibration-status').textContent=`标定状态：${calLabel} · 文件 ${cal.file_sha256||'—'} · 保存于 ${cal.saved_at||'—'}（修改 ${calTime(cal.file_mtime)}） · robotd ${(cal.robotd_sha256||'—').slice(0,8)} 启动于 ${calTime(cal.robotd_started)}${cal.error?' · '+cal.error:''}。robotd不记录已加载哪份标定，“已生效”按文件修改时间早于robotd启动时间推断。`;
$('calibration-status').className=cal.state==='applied'?'hint':'warn';
}catch(e){showError(e);}finally{statusBusy=false;}},250);

function navigationURL(target){
 const url=new URL('/navigate',location.href);url.port=document.querySelector('meta[name="setup-port"]').content;url.searchParams.set('target',target);url.searchParams.set('source','control');return url.href;
}
for(const [id,target] of [['nav-settings','setup'],['nav-calibration','calibration']]){
 const link=$(id);link.href=navigationURL(target);
 link.onclick=async event=>{event.preventDefault();if(pageClosing)return;pageClosing=true;keys.clear();resetSticks.forEach(reset=>reset());updateMotion();await releasePage();controlSocket?.close();location.assign(link.href);};
}
