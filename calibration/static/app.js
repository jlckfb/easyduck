import * as THREE from 'three';
import {OrbitControls} from './vendor/OrbitControls.js';
import {STLLoader} from './vendor/STLLoader.js';
const $=id=>document.getElementById(id), deg=x=>x*180/Math.PI;
let state=null,selected=20,mode='live',lastFrame=0,busy=false,ready=false,formDirty=false;
let root,manifest,home={},hinges=new Map(),meshes=[],geometry=new Map(),demoStart=0;
const scene=new THREE.Scene(); scene.background=new THREE.Color('#19212b');
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
function toast(msg,error=false){$('toast').textContent=msg;$('toast').classList.toggle('error',error);$('toast').style.display='block';clearTimeout(toast.timer);toast.timer=setTimeout(()=>$('toast').style.display='none',6500);}
async function command(d){
 const r=await fetch('/api/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(d),signal:AbortSignal.timeout(8000)});
 const data=await r.json();if(!r.ok)throw Error(data.error||'请求失败');return data;
}
async function run(d){try{const result=await command(d);toast(result.message,!!result.warning);await refresh();if(['preview','discard','solve','bias','capture_home_all'].includes(d.op)){formDirty=false;fillEditor();}return result;}catch(e){toast(e.message,true);return null;}}
function fresh(){return state && performance.now()-lastFrame<700;}
function choose(sid){if(formDirty && !confirm('此关节有尚未预览的编辑，切换会放弃这些编辑。继续？'))return;if(mode==='demo')setMode('live');selected=sid;formDirty=false;fillEditor();highlight();renderRows();}
function fillEditor(){
 if(!state)return;const row=state.rows.find(r=>r.id===selected),k=String(selected),p=state.profile;
 $('selectedName').textContent=`ID ${selected} · ${row.label}`;$('zero').value=p.zero[k];$('sign').value=p.sign[k];$('min').value=p.lim[k][0];$('max').value=p.lim[k][1];
 $('jointHelp').textContent=selected===34?'显示模型没有嘴部活动关节，可读数与标定；无法播放嘴部方向示范。':'示范只播放网页模型，不驱动实物。模型角度使用草稿参数。';
 $('demo').disabled=selected===34;updateReadouts();
}
function updateReadouts(){if(!state)return;const row=state.rows.find(r=>r.id===selected);$('raw').value=row.count??'无新鲜读数';$('degrees').value=!state.reference_aligned?'尚未设置参考HOME':row.angle==null?'未知':deg(row.angle).toFixed(1);}
function renderMotion(){
 const moving=!!state?.motion?.active,row=state?.rows.find(r=>r.id===selected);
 for(const id of ['stepBack','stepForward','testLimits'])$(id).disabled=busy||moving||!fresh()||row?.count==null;
 $('motionStatus').textContent=state?.motion?(state.motion.id?`ID ${state.motion.id} · `:'')+state.motion.message:'没有实物运动任务';
 if(state?.motion?.feedback_error_counts)$('motionStatus').textContent+=`；实际与目标相差 ${(Math.abs(state.motion.feedback_error_counts)*360/4096).toFixed(2)}°`;
 $('motionStatus').classList.toggle('error',!!state?.motion?.error);
 $('stopMotion').disabled=!moving;$('applyExit').disabled=busy||moving;$('preview').disabled=moving;$('discard').disabled=moving;$('save').disabled=moving;
 $('stopAllMotion').disabled=!moving;$('onAll').disabled=busy||moving||!fresh();$('homeAll').disabled=busy||moving||!fresh();
 $('captureHomeAll').disabled=busy||moving||!fresh();$('live').disabled=!state?.reference_aligned;
}
function renderRows(){
 if(!$('jointList').children.length)for(const r of state.rows){const el=document.createElement('div');el.className='joint';el.dataset.id=r.id;el.innerHTML=`<button class="pick">${r.label} <small>ID ${r.id} · ${r.name}</small></button><div class="reading"></div><button class="torque"></button>`;
 el.querySelector('.pick').onclick=()=>choose(r.id);
 el.querySelector('.torque').onclick=async()=>{if(!fresh()||busy)return;const current=state.rows.find(v=>v.id===r.id);if(current.torque==null)return;busy=true;renderRows();try{await run({op:'torque',id:r.id,on:!current.torque});}finally{busy=false;renderRows();}};
 $('jointList').append(el);}
 for(const r of state.rows){const el=$('jointList').querySelector(`[data-id="${r.id}"]`);el.classList.toggle('selected',r.id===selected);el.classList.toggle('bad',r.count==null);el.title=r.error||`${r.voltage} V / ${r.temperature} ℃ / ${r.current_ma?.toFixed(1)} mA`;
 el.querySelector('.reading').textContent=r.count==null?'离线':r.angle==null?`${r.count} / 未对齐`:`${r.count} / ${deg(r.angle).toFixed(1)}°`;
 const b=el.querySelector('.torque');b.textContent=r.torque==null?'状态未知':r.torque?'松力矩':'上力矩';b.classList.toggle('on',r.torque===true);b.disabled=busy||!fresh()||r.torque==null;}
}
async function refresh(){
 const response=await fetch('/api/state',{signal:AbortSignal.timeout(3000)});if(!response.ok)throw Error('读取反馈失败');
 const next=await response.json();const first=!state;state=next;home=next.home;lastFrame=performance.now();
 $('homeAlignment').textContent=next.reference_aligned?'已使用你记录的新HOME，可切换实时跟随对比':'旧HOME未用于实时3D；请摆好实物后点击“全部设为 HOME”';
 if(mode==='live'&&!next.reference_aligned)setMode('reference');
 $('connection').textContent=next.mock?'离线演示 · 非实物':`已连接 · ${next.rows.filter(r=>r.count!=null).length}/15 在线`;
 $('rate').textContent=`${next.hz} Hz`;$('dirty').textContent=next.dirty?'有未保存草稿':'与已保存参数一致';
 const rt=next.runtime_support||{},rtSha=(rt.sha256||'未知').slice(0,8);
 $('runtime').textContent=rt.supported?`正式程序 robotd ${rtSha} 会读网页标定（${rt.reason}），在启动时读取：保存后点“退出标定并应用到正式控制”才生效。修改舵机方向后，需把导出的标定交给训练开发同步。现在为标定模式，行走服务停止。`:`当前正式程序（robotd ${rtSha}）不读网页标定，保存不会影响网页控制。${rt.reason&&rt.reason!=='当前正式程序不读网页标定'?'（'+rt.reason+'）':''}`;
 Object.assign($('runtime').style,rt.supported?{borderLeftColor:'',background:'',color:''}:{borderLeftColor:'#e0715f',background:'#4a2522',color:'#ffd6cf'});
 $('poseStatus').textContent=Object.keys(next.poses).length?'已采集：'+Object.keys(next.poses).map(p=>({level:'站直',front_down:'前倾',left_down:'左倾',front_up:'后倾',right_down:'右倾'}[p])).join('、'):'尚未采集姿态';
 if(next.imu){const q=quat(next.imu.body_quat),e=new THREE.Euler().setFromQuaternion(q,'ZYX');$('imuReadout').textContent=`侧倾 ${deg(e.x).toFixed(1)}°　俯仰 ${deg(e.y).toFixed(1)}°　相对航向 ${deg(e.z).toFixed(1)}° ｜ 重力 [${next.imu.gravity.map(v=>v.toFixed(2)).join(', ')}] ｜ 转速 [${next.imu.body_gyro.map(v=>v.toFixed(3)).join(', ')}] rad/s`;}else $('imuReadout').textContent='IMU无新鲜反馈，身体姿态不再更新';
 renderRows();renderMotion();if(first)fillEditor();else updateReadouts();
}
async function poll(){try{await refresh();}catch(e){$('connection').textContent='反馈连接中断';}setTimeout(poll,65);}
function animate(t){requestAnimationFrame(animate);const ok=fresh();$('stale').style.display=ok?'none':'block';$('stale').textContent='反馈中断 · 已冻结显示';
 if(mode==='demo'&&t-demoStart>=2000)setMode('live');
 if(!ok && state)for(const b of document.querySelectorAll('.torque,#stepBack,#stepForward,#testLimits'))b.disabled=true;
 if(ready && state){for(const [name,h] of hinges){let value=home[name]??0;
  if(mode==='live'){const r=state.rows.find(r=>r.name===name);if(!ok||r?.angle==null)continue;value=r.angle;}
  if(mode==='demo'&&name===state.rows.find(r=>r.id===selected)?.name)value+=Math.PI/12*(.5-.5*Math.cos((t-demoStart)/1000*Math.PI));
  h.pivot.quaternion.setFromAxisAngle(h.axis,value);}
  if(mode!=='live'||!$('followImu').checked)root.quaternion.identity();
  else if(ok&&state.imu)root.quaternion.copy(quat(state.imu.body_quat));
 }
 controls.update();renderer.render(scene,camera);
}
function setMode(m){if(m==='live'&&!state?.reference_aligned)m='reference';mode=m;demoStart=performance.now();$('modeLabel').textContent=m==='live'?'实物实时跟随':m==='reference'?'训练参考站姿 · 固定显示':'正方向示范 · 只动模型';$('demo').textContent=m==='demo'?'停止正方向示范':'正方向示范 +15°';$('stopDemo').hidden=m!=='demo';}
$('live').onclick=()=>setMode('live');$('reference').onclick=()=>setMode('reference');$('demo').onclick=()=>setMode(mode==='demo'?'live':'demo');$('stopDemo').onclick=()=>setMode('live');$('resetCamera').onclick=resetCamera;
async function moveSelected(op,d={}){
 if(!fresh())return toast('没有新鲜反馈，请等待连接恢复',true);
 if(formDirty)return toast('先点击“预览此关节”，再执行步进或测试',true);
 if(busy||state.motion?.active)return;
 const speed=Number($('motionSpeed').value);
 if(op==='test_limits'&&!confirm(`将让ID ${selected} 上力，按已预览范围缓慢运行：计数上限 → 下限 → 中间。请确保关节可自由运动；可随时点“停止并保持”或“全部松力矩”。`))return;
 busy=true;renderMotion();try{const result=await run({op,id:selected,revision:state.revision,speed,...d});if(result)setMode('live');}finally{busy=false;renderMotion();}
}
$('stepForward').onclick=()=>moveSelected('step',{degrees:Number($('stepDegrees').value)});
$('stepBack').onclick=()=>moveSelected('step',{degrees:-Number($('stepDegrees').value)});
$('testLimits').onclick=()=>moveSelected('test_limits');$('stopMotion').onclick=()=>run({op:'stop_motion'});
$('stopAllMotion').onclick=()=>run({op:'stop_motion'});
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
$('captureZero').onclick=()=>capture('zero');$('captureMin').onclick=()=>capture('min');$('captureMax').onclick=()=>capture('max');
$('preview').onclick=()=>{if(!state)return;const p=structuredClone(state.profile),k=String(selected);p.zero[k]=Number($('zero').value);p.sign[k]=Number($('sign').value);p.lim[k]=[Number($('min').value),Number($('max').value)];run({op:'preview',profile:p,revision:state.revision});};
$('discard').onclick=()=>run({op:'discard'});
$('save').onclick=()=>{if(formDirty)return toast('先点击“预览此关节”，再保存',true);if(!state?.dirty)return toast('参数没有变化，无需保存');if(confirm('将保存全部草稿，保留旧文件备份。'+(state.runtime_support?.supported?'正式程序启动时读取，保存后点“退出标定并应用到正式控制”生效。':'当前正式程序不读网页标定，保存不会影响网页控制。')+'确认前请先核对实物方向和姿态。'))run({op:'save',revision:state.revision});};
$('applyExit').onclick=async()=>{
 if(!state||busy)return;
 if(formDirty||state.dirty)return toast('有未保存草稿：请先保存或撤销，再退出标定',true);
 if(state.motion?.active)return toast('请先停止实物运动',true);
 const rt=state.runtime_support||{};
 if(!confirm('确认机器人已躺好或已扶住，且15个舵机已全部断力？\n\n将依次：停止标定 → 启动robotd → 启动网页控制（9527）。robotd启动后处于xgo_off，不会上力。本页随后断开。'+(rt.supported?'':'\n\n注意：当前robotd不读网页标定，保存的标定不会生效。')))return;
 busy=true;renderMotion();try{await run({op:'apply_exit',revision:state.revision,confirm:true});}finally{busy=false;renderMotion();}
};
$('export').onclick=()=>{if(!state)return;const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify(state.profile,null,2)],{type:'application/json'}));a.download='xgoduck-calibration.json';a.click();URL.revokeObjectURL(a.href);};
for(const b of document.querySelectorAll('[data-pose]'))b.onclick=()=>run({op:'pose',pose:b.dataset.pose});
$('solve').onclick=()=>run({op:'solve'});$('bias').onclick=()=>run({op:'bias'});
const ray=new THREE.Raycaster(),mouse=new THREE.Vector2();let down;
renderer.domElement.addEventListener('pointerdown',e=>down=[e.clientX,e.clientY]);renderer.domElement.addEventListener('pointerup',e=>{if(!down||Math.hypot(e.clientX-down[0],e.clientY-down[1])>5)return;const r=renderer.domElement.getBoundingClientRect();mouse.set((e.clientX-r.left)/r.width*2-1,-(e.clientY-r.top)/r.height*2+1);ray.setFromCamera(mouse,camera);const hit=ray.intersectObjects(meshes).find(h=>h.object.userData.joint);if(hit&&state){const row=state.rows.find(v=>v.name===hit.object.userData.joint);if(row)choose(row.id);}});
loadModel().catch(e=>{$('modelStatus').textContent='模型加载失败';toast(e.message,true);});poll();requestAnimationFrame(animate);
window.calibrationDebug=()=>({ready,meshCount:meshes.length,jointCount:hinges.size,selected,mode,fresh:fresh(),state});
