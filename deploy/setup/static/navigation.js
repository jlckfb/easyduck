'use strict';
const $=id=>document.getElementById(id);
const target=document.querySelector('meta[name="navigation-target"]').content;
const csrf=document.querySelector('meta[name="csrf-token"]').content;
try{document.documentElement.dataset.theme=localStorage.getItem('easyduck-theme')||'night';}catch(_){}
const names={setup:'连接设置',control:'机器人控制',calibration:'实物标定台',servo_setup:'新机舵机 ID 分配'};
$('title').textContent='切换到'+names[target];
const back=new URL('/',location.origin);back.port=document.querySelector('meta[name="source-port"]').content;$('back').href=back.href;
let busy=false;
function error(message){const node=document.createElement('div');node.className='toast err';node.textContent=message;$('toasts').replaceChildren(node);$('retry').hidden=false;}
async function request(path,options){
 try{const r=await fetch(path,{cache:'no-store',...options,signal:AbortSignal.timeout(12000)});const data=await r.json();if(!r.ok)throw Error(data.message||'切换失败，请重试。');return data;}
 catch(e){if(e instanceof TypeError||['AbortError','TimeoutError'].includes(e.name))throw Error('网络连接中断或等待超时，请确认当前无线地址可访问后重试。');throw e;}
}
async function navigate(){
 if(busy)return;busy=true;$('continue').disabled=true;$('retry').hidden=true;$('toasts').replaceChildren();
 $('explanation').textContent='正在检查并切换服务，请稍候…';
 try{
  const started=await request('/api/navigation',{method:'POST',headers:{'Content-Type':'application/json','X-CSRF-Token':csrf},body:JSON.stringify({target,confirm:['calibration','servo_setup'].includes(target)})});
  const end=Date.now()+120000;let result;
  while(Date.now()<end){await new Promise(resolve=>setTimeout(resolve,500));const job=await request('/api/jobs/'+started.job);if(job.done){result=job.result;break;}}
  if(!result)throw Error('切换等待超时，请检查服务状态后重试。');if(!result.ok)throw Error(result.message);
  const url=new URL('/',location.origin);if(result.host)url.hostname=result.host;url.port=result.port===null?location.port:String(result.port);url.pathname=result.path;location.replace(url.href);
 }catch(e){error(e.message);$('explanation').textContent='页面尚未切换，请处理提示后重试。';}
 finally{busy=false;$('continue').disabled=false;}
}
$('continue').onclick=navigate;$('retry').onclick=navigate;
if(target==='servo_setup'){$('explanation').textContent='请放稳机器人，每次只新增一只待编号舵机，已编号且 ID 不重复的舵机可保留连接。将停止正式控制并进入标定台的 ID 页签，不会上力或修改零点、P/D、限流。';$('continue').textContent='确认并进入 ID 设置';$('continue').hidden=false;}else if(target==='calibration'){$('explanation').textContent='进入标定会停止正式控制并解除全部舵机力矩。请先放稳或托住机器人，给关节留出空间。切换不会上力、执行动作或修改标定。';$('continue').hidden=false;}else navigate();
