import * as THREE from 'three';
import {OrbitControls} from './vendor/OrbitControls.js';
import {STLLoader} from './vendor/STLLoader.js';
const $=id=>document.getElementById(id), deg=x=>x*180/Math.PI;
let state=null,selected=20,mode='live',lastFrame=0,busy=false,ready=false,formDirty=false;
let root,manifest,home={},hinges=new Map(),meshes=[],geometry=new Map(),demoStart=0;
const scene=new THREE.Scene(); scene.background=new THREE.Color(getComputedStyle(document.documentElement).getPropertyValue('--scene').trim()||'#101c17');
const camera=new THREE.PerspectiveCamera(35,1,.001,30);camera.up.set(0,0,1);
const renderer=new THREE.WebGLRenderer({antialias:true});renderer.setPixelRatio(Math.min(devicePixelRatio,2));
renderer.outputColorSpace=THREE.SRGBColorSpace;$('viewport').append(renderer.domElement);
const controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true;
scene.add(new THREE.HemisphereLight(0xe9f6ff,0x303f36,2.4));
const light=new THREE.DirectionalLight(0xffffff,2.2);light.position.set(1,-1,2);scene.add(light);
const grid=new THREE.GridHelper(1,20,0x496056,0x2d3c49);grid.rotation.x=Math.PI/2;scene.add(grid);
const axes=new THREE.AxesHelper(.16);axes.position.set(-.22,-.2,.001);scene.add(axes);
function resetCamera(){camera.position.set(.44,-.52,.33);controls.target.set(0,0,.15);controls.update();}
resetCamera();new ResizeObserver(()=>{const v=$('viewport');renderer.setSize(v.clientWidth,v.clientHeight);camera.aspect=v.clientWidth/v.clientHeight;camera.updateProjectionMatrix();}).observe($('viewport'));
const quat=q=>new THREE.Quaternion(q[1],q[2],q[3],q[0]).normalize();
function makeBody(b,parent){
 const frame=new THREE.Group();frame.position.fromArray(b.pos);frame.quaternion.copy(quat(b.quat));parent.add(frame);
 let content=frame,owner=b.joint?.name;
 if(b.joint){const pivot=new THREE.Group();pivot.position.fromArray(b.joint.pos);frame.add(pivot);
  content=new THREE.Group();content.position.fromArray(b.joint.pos).multiplyScalar(-1);pivot.add(content);
  hinges.set(owner,{pivot,axis:new THREE.Vector3(...b.joint.axis).normalize()});}
 for(const g of b.geoms){const mesh=new THREE.Mesh(geometry.get(g.mesh),new THREE.MeshStandardMaterial({color:owner?.startsWith('left')?0x7cabb9:owner?.startsWith('right')?0xadb8c9:0xced5d8,metalness:.16,roughness:.65}));
  mesh.position.fromArray(g.pos);mesh.quaternion.copy(quat(g.quat));mesh.scale.fromArray(manifest.meshes[g.mesh].scale);mesh.userData.joint=owner;mesh.userData.base=mesh.material.color.clone();content.add(mesh);meshes.push(mesh);}
 for(const child of b.children)makeBody(child,content);
 return frame;
}
async function loadModel(){
 manifest=await fetch('/model/manifest.json').then(r=>r.json()); const loader=new STLLoader();
 await Promise.all(Object.entries(manifest.meshes).map(async([name,m])=>geometry.set(name,await loader.loadAsync('/model/'+m.file))));
 root=new THREE.Group();root.position.z=.15;scene.add(root);makeBody(manifest.root,root);ready=true;
 $('modelStatus').textContent='3D显示模型 · 14关节 · 资源已完整加载';highlight();
}
function highlight(){for(const m of meshes)m.material.color.copy(m.userData.joint===state?.rows.find(r=>r.id===selected)?.name?new THREE.Color(0x7cf5b9):m.userData.base);}
function toast(msg,error=false){const el=$('toast');el.replaceChildren();const close=document.createElement('button');close.textContent='×';close.setAttribute('aria-label','关闭提示');close.onclick=()=>el.style.display='none';el.append(close,document.createTextNode(msg));el.classList.toggle('error',error);el.style.display='block';clearTimeout(toast.timer);if(!error)toast.timer=setTimeout(()=>el.style.display='none',6500);}

const feed=window.calibrationFeed;
const command=d=>feed.command(d);
async function run(d){try{const result=await command(d);toast(result.message,!!result.warning);await refresh();if(['preview','discard','solve','bias','capture_home_all','capture_home_one','reset_limits','confirm_limits','center_servo'].includes(d.op)){formDirty=false;fillEditor();}return result;}catch(e){toast(e.message,true);return null;}}
function fresh(){return state && feed.age<1500&&!feed.busy;}
function choose(sid){if(formDirty && !confirm('此关节有未保存的编辑，切换会放弃当前表单修改。继续？'))return;if(mode==='demo')setMode('live');selected=sid;formDirty=false;fillEditor();highlight();renderRows();}
function fillEditor(){
 if(!state)return;const row=state.rows.find(r=>r.id===selected),k=String(selected),p=state.profile;
 $('railJoint').textContent='ID '+selected;$('jointSelect').value=String(selected);$('selectedName').textContent=`ID ${selected} · ${row.label}`;$('zero').value=p.zero[k];$('sign').value=p.sign[k];$('min').value=p.lim[k][0];$('max').value=p.lim[k][1];
 $('jointHelp').textContent=selected===34?'显示模型没有嘴部活动关节，可读数与标定；无法播放嘴部方向示范。':'示范只播放网页模型，不驱动实物。模型角度使用草稿参数。';
 $('demo').disabled=selected===34;updateReadouts();
}
function updateReadouts(){if(!state)return;const row=state.rows.find(r=>r.id===selected);$('raw').value=row.count??'无新鲜读数';$('rawSigned').value=row.count==null?'无新鲜读数':row.position_signed??row.count;$('turnHint').textContent=row.count==null?'等待有效位置反馈':`单圈位置 ${row.count} / 4095${(row.position_signed??row.count)<0||(row.position_signed??row.count)>4095?' · 存在累计圈数，可断力后清除':' · 无需清圈'}`;$('degrees').value=!state.reference_aligned?'未记录站姿':row.angle==null?'未知':deg(row.angle).toFixed(1);}
function renderMotion(){
 const moving=!!state?.motion?.active,row=state?.rows.find(r=>r.id===selected);
 for(const id of ['stepBack','stepForward','testLimits','captureZero','captureMin','captureMax','selectedTorque','homeSelected'])$(id).disabled=busy||moving||!fresh()||row?.count==null;
 const pending=state?.profile?.limits_pending||[],unconfirmed=pending.includes(selected);
 $('limitStatus').textContent=unconfirmed?'新机器人：机械限位尚未确认；0～4095仅为编码器范围，可先小步调整。':'使用当前设置的计数上下限';
 $('testLimits').disabled=$('testLimits').disabled||unconfirmed;
 $('selectedTorque').disabled=$('selectedTorque').disabled||row?.torque==null;
 $('resetTurns').disabled=busy||moving||!fresh()||row?.count==null||row?.torque!==false;
 $('centerServo').disabled=busy||moving||!fresh()||row?.torque!==false||!unconfirmed;
 $('resetLimits').disabled=busy||moving||!fresh();$('confirmLimits').disabled=busy||moving||!fresh()||!unconfirmed;
 $('motionStatus').textContent=state?.motion?(state.motion.id?`ID ${state.motion.id} · `:'')+state.motion.message:'没有实物运动任务';
 if(state?.motion?.feedback_error_counts)$('motionStatus').textContent+=`；实际与目标相差 ${(Math.abs(state.motion.feedback_error_counts)*360/4096).toFixed(2)}°`;
 $('motionStatus').classList.toggle('error',!!state?.motion?.error||!!state?.motion?.warning);
 $('stopMotion').disabled=!moving;$('applyExit').disabled=busy||moving;$('discard').disabled=moving;$('save').disabled=busy||moving;$('saveAll').disabled=busy||moving;$('save').hidden=document.body.classList.contains('imu-mode');
 $('onAll').disabled=busy||moving||!fresh();$('homeAll').disabled=busy||moving||!fresh();
 $('captureHomeAll').disabled=busy||moving||!fresh();$('live').disabled=!state?.reference_aligned;
}
function renderRows(){
 if(!$('jointSelect').options.length)for(const r of state.rows){const o=document.createElement('option');o.value=r.id;o.textContent=`${r.label} · ID ${r.id}`;$('jointSelect').append(o);}
 if(!$('jointList').children.length)for(const r of state.rows){const el=document.createElement('div');el.className='joint';el.dataset.id=r.id;el.innerHTML=`<button class="pick">${r.label} <small>ID ${r.id} · ${r.name}</small></button><div class="reading"></div>`;
 el.querySelector('.pick').onclick=()=>choose(r.id);
 $('jointList').append(el);}
 for(const r of state.rows){const el=$('jointList').querySelector(`[data-id="${r.id}"]`);el.classList.toggle('selected',r.id===selected);el.classList.toggle('bad',r.count==null);el.title=r.error||`${r.voltage} V / ${r.temperature} ℃ / ${r.current_ma?.toFixed(1)} mA`;
 el.querySelector('.reading').textContent=r.count==null?(r.error?.includes('超时')?'离线 · 读取超时':r.error||'无反馈'):`${r.position_signed??r.count} · ${r.torque?'上力':'松开'}`;
 }
 const row=state.rows.find(r=>r.id===selected);$('selectedTorque').textContent=row?.torque?'松开此关节':'上力保持当前位置';$('selectedTorque').disabled=busy||!fresh()||row?.torque==null||state.motion?.active;

}
async function refresh(){
 return feed.read();
}
function applyState(next){
 const first=!state;state=next;home=next.home;lastFrame=performance.now();
 $('homeAlignment').textContent=next.reference_aligned?'已记录整机站姿 · 可跟随实物对比':'尚未记录整机站姿 · 3D显示参考姿态';
 if(mode==='live'&&!next.reference_aligned)setMode('reference');
 $('connection').textContent=next.mock?'离线演示 · 非实物':`已连接 · ${next.rows.filter(r=>r.count!=null).length}/15 在线`;
 $('rate').textContent=`${next.hz} Hz`;$('dirty').textContent=next.dirty?'有未保存草稿':'与已保存参数一致';
 const rt=next.runtime_support||{},rtSha=(rt.sha256||'未知').slice(0,8);
 $('runtime').textContent=rt.supported?`正式程序 robotd ${rtSha} 会读网页标定（${rt.reason}），在启动时读取：保存后点“退出标定并应用到正式控制”才生效。修改舵机方向后，需把导出的标定交给训练开发同步。现在为标定模式，行走服务停止。`:`当前正式程序（robotd ${rtSha}）不读网页标定，保存不会影响网页控制。${rt.reason&&rt.reason!=='当前正式程序不读网页标定'?'（'+rt.reason+'）':''}`;
 Object.assign($('runtime').style,rt.supported?{borderLeftColor:'',background:'',color:''}:{borderLeftColor:'#e0715f',background:'#4a2522',color:'#ffd6cf'});
 $('poseStatus').textContent=Object.keys(next.poses).length?'已采集：'+Object.keys(next.poses).map(p=>({level:'站直',front_down:'前倾',left_down:'左倾',front_up:'后倾',right_down:'右倾'}[p])).join('、'):'尚未采集姿态';
 const device=next.imu_status;
 if(device){const ages=device.ages_ms||{},age=k=>ages[k]===undefined?'无数据':`${ages[k]} ms`;const caps=device.capabilities||{};
 $('imuDevice').textContent=`${device.model} / ${device.fusion} ｜ ${device.connected?'已连接':'未连接'} ｜ 姿态 ${age('quat')} / 角速度 ${age('gyro')} / 加速度 ${age('accel')} ｜ 能力：${['orientation','gyro','accel'].filter(k=>caps[k]).map(k=>({orientation:'芯片姿态',gyro:'角速度',accel:'加速度'}[k])).join('、')||'等待确认'} ｜ ${device.calibration_valid?'已有安装标定':'此型号尚未标定安装方向'}${device.error?' ｜ '+device.error:''}`;
 document.querySelectorAll('[data-pose]').forEach(b=>b.disabled=!device.fresh);$('bias').disabled=!device.fresh;}
 if(next.imu?.body_quat){const q=quat(next.imu.body_quat),e=new THREE.Euler().setFromQuaternion(q,'ZYX');$('imuReadout').textContent=`侧倾 ${deg(e.x).toFixed(1)}°　俯仰 ${deg(e.y).toFixed(1)}°　相对航向 ${deg(e.z).toFixed(1)}° ｜ 重力 [${next.imu.gravity.map(v=>v.toFixed(2)).join(', ')}] ｜ 转速 [${next.imu.body_gyro.map(v=>v.toFixed(3)).join(', ')}] rad/s`;}else $('imuReadout').textContent=next.imu?'传感器数据有效，请采集姿态并计算此型号的安装方向':'IMU无新鲜反馈，身体姿态不再更新';
 renderRows();renderMotion();if(first)fillEditor();else updateReadouts();
}
feed.subscribe(applyState);
feed.onError(e=>{$('connection').textContent='连接暂不可用 · 正在重试';$('connection').title=e.message;renderMotion();});
document.addEventListener('calibration-view-error',e=>toast('页面显示错误：'+e.detail,true));
document.addEventListener('calibration-command-state',()=>{if(state)renderMotion();});
function animate(t){requestAnimationFrame(animate);const ok=fresh();$('stale').style.display=ok||feed.busy?'none':'block';$('stale').textContent='反馈中断 · 已冻结显示';
 if(mode==='demo'&&t-demoStart>=2000)setMode('live');
 if(!ok && state)for(const b of document.querySelectorAll('.torque,#stepBack,#stepForward,#testLimits,#homeSelected'))b.disabled=true;
 if(ready && state){for(const [name,h] of hinges){let value=home[name]??0;
  if(mode==='live'){const r=state.rows.find(r=>r.name===name);if(!ok||r?.angle==null)continue;value=r.angle;}
  if(mode==='demo'&&name===state.rows.find(r=>r.id===selected)?.name)value+=Math.PI/12*(.5-.5*Math.cos((t-demoStart)/1000*Math.PI));
  h.pivot.quaternion.setFromAxisAngle(h.axis,value);}
  if(mode!=='live'||!$('followImu').checked)root.quaternion.identity();
  else if(ok&&state.imu?.body_quat)root.quaternion.copy(quat(state.imu.body_quat));
 }
 controls.update();renderer.render(scene,camera);
}
function setMode(m){if(m==='live'&&!state?.reference_aligned)m='reference';mode=m;demoStart=performance.now();$('modeLabel').textContent=m==='live'?'实物实时跟随':m==='reference'?'参考站姿 · 只显示模型':'正方向示范 · 只动模型';$('demo').textContent=m==='demo'?'停止模型示范':'看模型正方向';$('stopDemo').hidden=m!=='demo';}
$('live').onclick=()=>setMode('live');$('reference').onclick=()=>setMode('reference');$('demo').onclick=()=>setMode(mode==='demo'?'live':'demo');$('stopDemo').onclick=()=>setMode('live');$('resetCamera').onclick=resetCamera;
async function moveSelected(op,d={}){
 if(!fresh())return toast('没有新鲜反馈，请等待连接恢复',true);
 if(formDirty&&!await previewEditor())return;
 if(busy||state.motion?.active)return;
 const speed=Number($('motionSpeed').value);
 if(op==='home_one'&&!confirm(`将使 ID ${selected} 上力并缓慢回到这只关节已保存的 HOME。请留出活动空间；可随时停止或松力矩。`))return;
 if(op==='test_limits'&&!confirm(`将让ID ${selected} 上力，按当前设置的范围缓慢运行：计数上限 → 下限 → 中间。请确保关节可自由运动；可随时点“停止并保持”或“全部松力矩”。`))return;
 busy=true;renderMotion();try{const result=await run({op,id:selected,revision:state.revision,speed,...d});if(result)setMode('live');}finally{busy=false;renderMotion();}
}
$('homeSelected').onclick=()=>moveSelected('home_one');
$('stepForward').onclick=()=>moveSelected('step',{degrees:Number($('stepDegrees').value)});
$('stepBack').onclick=()=>moveSelected('step',{degrees:-Number($('stepDegrees').value)});
$('resetTurns').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 const row=state.rows.find(r=>r.id===selected);
 if(row?.torque!==false)return toast('请先松开此关节的力矩',true);
 if(!confirm(`仅清除 ID ${selected} 的累计圈数（当前 ${row.position_signed??row.count}，单圈 ${row.count}）？不会上力或转动，不改变硬件中位、HOME、方向或活动范围。若工作行程跨过编码器零点，清圈后仍会再次跨零。`))return;
 busy=true;renderMotion();try{await run({op:'reset_turns',id:selected,revision:state.revision,confirmed:true});}finally{busy=false;renderMotion();}
};
$('centerServo').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 if(formDirty&&!await previewEditor())return;
 if(!confirm(`确认 ID ${selected} 已断力，并已手动摆到正确的安装中位？此操作会永久重设这只舵机的硬件中位到2048附近，不上力、不转动、不改P/D和限流。原软件HOME将失效，需要重新记录整机HOME。`))return;
 busy=true;renderMotion();try{await run({op:'center_servo',id:selected,revision:state.revision,confirmed:true});}finally{busy=false;renderMotion();}
};
$('resetLimits').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 const pendingHome=formDirty?{zero:$('zero').value,sign:$('sign').value}:null;
 if(!confirm('这是新机器人？将备份并清除全部旧限位，保留方向与HOME草稿，不执行动作。0～4095仅是编码器范围，不代表机械行程。确认限位前禁止自动上下限测试。'))return;
 busy=true;renderMotion();try{const result=await run({op:'reset_limits',revision:state.revision,confirmed:true});if(result&&pendingHome){$('zero').value=pendingHome.zero;$('sign').value=pendingHome.sign;formDirty=true;}}finally{busy=false;renderMotion();}
};
$('confirmLimits').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 if(formDirty&&!await previewEditor())return;
 const [lo,hi]=state.profile.lim[String(selected)];
 if(!confirm(`确认 ID ${selected} 的 ${lo}～${hi} 是你已核对的机械安全范围？不要把编码器全范围直接当机械范围。确认后才启用自动上下限测试。`))return;
 busy=true;renderMotion();try{await run({op:'confirm_limits',id:selected,revision:state.revision});}finally{busy=false;renderMotion();}
};
$('testLimits').onclick=()=>moveSelected('test_limits');$('stopMotion').onclick=()=>run({op:'stop_motion'});

$('onAll').onclick=async()=>{if(!fresh()||busy||state.motion?.active)return;busy=true;renderMotion();try{await run({op:'on_all'});}finally{busy=false;renderMotion();}};
$('homeAll').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 if(formDirty||state.dirty)return toast('请先保存标定或撤销草稿，再回已保存的HOME',true);
 if(!confirm('将使15个舵机上力，缓慢回到你保存的HOME零点。请承托好机器人并留出活动空间；途中可停止或全部松力矩。'))return;
 busy=true;renderMotion();try{const result=await run({op:'home',revision:state.revision,speed:Number($('motionSpeed').value)});if(result)setMode('live');}finally{busy=false;renderMotion();}
};
$('offAll').onclick=()=>run({op:'off_all'});
$('captureHomeAll').onclick=async()=>{
 if(!fresh()||busy||state.motion?.active)return;
 if(formDirty||state.dirty)return toast('请先保存或撤销已有编辑，再记录全部HOME',true);
 if(!confirm('确认实物已摆成训练参考站姿？将读取15个舵机当前位置，保存为新的HOME并备份已有标定。此操作不改变力矩、不执行运动。'))return;
 busy=true;renderMotion();try{const result=await run({op:'capture_home_all',revision:state.revision});if(result)setMode('live');}finally{busy=false;renderMotion();}
};
for(const id of ['zero','sign','min','max'])$(id).oninput=()=>formDirty=true;
function capture(field){if(!fresh())return toast('反馈已过期',true);const r=state?.rows.find(r=>r.id===selected);if(r?.count==null)return toast('没有有效读数',true);$(field).value=r.count;formDirty=true;}
$('captureZero').onclick=async()=>{if(!fresh()||busy||state.motion?.active)return;if(formDirty&&!await previewEditor())return;busy=true;renderMotion();try{await run({op:'capture_home_one',id:selected,revision:state.revision});}finally{busy=false;renderMotion();}};
$('captureMin').onclick=async()=>{capture('min');if(formDirty)await previewEditor();};$('captureMax').onclick=async()=>{capture('max');if(formDirty)await previewEditor();};
async function previewEditor(){if(!state)return;const p=structuredClone(state.profile),k=String(selected);p.zero[k]=Number($('zero').value);p.sign[k]=Number($('sign').value);p.lim[k]=[Number($('min').value),Number($('max').value)];return await run({op:'preview',profile:p,revision:state.revision});}
$('discard').onclick=()=>{if(formDirty&&!state?.dirty){formDirty=false;fillEditor();toast('已撤销当前编辑');}else run({op:'discard'});};
async function saveCalibration(all){
 if(!state||busy||state.motion?.active)return;
 if(formDirty&&!await previewEditor())return;
 if(!state.dirty)return toast('参数没有变化，无需保存');
 busy=true;renderMotion();
 try{await run({op:all?'save':'save_one',...(all?{}:{id:selected}),revision:state.revision});}finally{busy=false;renderMotion();}
}
$('save').onclick=()=>saveCalibration(false);
$('saveAll').onclick=()=>saveCalibration(true);
$('applyExit').onclick=()=>navigateTo('control');
$('export').onclick=()=>{if(!state)return;const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify(state.profile,null,2)],{type:'application/json'}));a.download='xgoduck-calibration.json';a.click();URL.revokeObjectURL(a.href);};
for(const b of document.querySelectorAll('[data-pose]'))b.onclick=()=>run({op:'pose',pose:b.dataset.pose});
$('solve').onclick=()=>run({op:'solve'});$('bias').onclick=()=>run({op:'bias'});
const ray=new THREE.Raycaster(),mouse=new THREE.Vector2();let down;
renderer.domElement.addEventListener('pointerdown',e=>down=[e.clientX,e.clientY]);renderer.domElement.addEventListener('pointerup',e=>{if(!down||Math.hypot(e.clientX-down[0],e.clientY-down[1])>5)return;const r=renderer.domElement.getBoundingClientRect();mouse.set((e.clientX-r.left)/r.width*2-1,-(e.clientY-r.top)/r.height*2+1);ray.setFromCamera(mouse,camera);const hit=ray.intersectObjects(meshes).find(h=>h.object.userData.joint);if(hit&&state){const row=state.rows.find(v=>v.name===hit.object.userData.joint);if(row)choose(row.id);}});
loadModel().catch(e=>{$('modelStatus').textContent='模型加载失败';toast(e.message,true);});requestAnimationFrame(animate);
window.calibrationDebug=()=>({ready,meshCount:meshes.length,jointCount:hinges.size,selected,mode,fresh:fresh(),state});

function navigationURL(target){
 const url=new URL('/navigate',location.href);url.port=document.querySelector('meta[name="setup-port"]').content;url.searchParams.set('target',target);url.searchParams.set('source','calibration');return url.href;
}
function navigateTo(target){
 if(!state||busy)return toast('请等待当前操作完成后再切换页面',true);
 if(formDirty||state.dirty)return toast('有未保存草稿：请先保存或撤销，再切换页面',true);
 if(state.motion?.active)return toast('请先停止实物运动',true);
 if(state.rows.some(row=>row.torque!==false||row.error))return toast('请先全部松力矩，并确认全部舵机反馈正常',true);
 location.assign(navigationURL(target));
}
for(const [id,target] of [['nav-settings','setup'],['nav-control','control']]){
 const link=$(id);link.href=navigationURL(target);link.onclick=event=>{event.preventDefault();navigateTo(target);};
}
window.addEventListener('beforeunload',event=>{if(formDirty||state?.dirty){event.preventDefault();event.returnValue='';}});

$('selectedTorque').onclick=async()=>{const row=state?.rows.find(r=>r.id===selected);if(!fresh()||busy||row?.torque==null)return;busy=true;renderMotion();try{await run({op:'torque',id:selected,on:!row.torque});}finally{busy=false;renderMotion();renderRows();}};
$('tab-imu').onclick=()=>{if(state?.id_setup?.active)return toast('请先返回关节标定，再切换IMU',true);document.body.classList.add('imu-mode');$('tab-imu').setAttribute('aria-selected','true');$('tab-calibration').setAttribute('aria-selected','false');};
$('tab-calibration').addEventListener('click',()=>{document.body.classList.remove('imu-mode');$('tab-imu').setAttribute('aria-selected','false');});
$('tab-servo-id').addEventListener('click',()=>{document.body.classList.remove('imu-mode');$('tab-imu').setAttribute('aria-selected','false');});

$('jointSelect').onchange=()=>{const next=Number($('jointSelect').value);choose(next);$('jointSelect').value=String(selected);};

new MutationObserver(()=>{scene.background.set(getComputedStyle(document.documentElement).getPropertyValue('--scene').trim());}).observe(document.documentElement,{attributes:true,attributeFilter:['data-theme']});
