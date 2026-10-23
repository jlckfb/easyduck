(() => {
 const $=id=>document.getElementById(id);
 const joints=[[10,'右髋转向'],[11,'右髋侧摆'],[12,'右髋前后'],[13,'右膝'],[14,'右踝'],[20,'左髋转向'],[21,'左髋侧摆'],[22,'左髋前后'],[23,'左膝'],[24,'左踝'],[30,'颈俯仰'],[31,'头俯仰'],[32,'头转向'],[33,'头侧摆'],[34,'嘴部']];
 for(const [id,label] of joints){const o=document.createElement('option');o.value=id;o.textContent=`${label} · ID ${id}`;$('id-joint').append(o);}
 let state=null,pending=false,lastSeen=0,disconnected=false;
 const feed=window.calibrationFeed;
 const command=body=>feed.command(body);
 function show(message,error=false){$('id-result').textContent=message;$('id-result').style.color=error?'#ffb9a9':'var(--accent)';}
 function render(){
  if(!state)return;
  const active=state.active;document.body.classList.toggle('id-mode',active);$('servo-id-panel').hidden=!active;
  $('tab-calibration').setAttribute('aria-selected',String(!active&&!document.body.classList.contains('imu-mode')));$('tab-servo-id').setAttribute('aria-selected',String(active));
  $('id-progress').value=state.checked;
  $('id-status').textContent=state.error?`扫描已停止：${state.error}`:`${state.scanning?'正在扫描':state.complete?'扫描完成':'扫描未完成或已停止'} · ${state.checked}/254 · 找到 ${state.found.length} 个 ID`;
  const old=$('id-found').dataset.ids,ids=state.found.join(',')+';'+(state.selected?.id??'');
  if(old!==ids){$('id-found').dataset.ids=ids;$('id-found').replaceChildren();for(const id of state.found){const b=document.createElement('button');b.textContent=`ID ${id}`;b.setAttribute('aria-pressed',String(state.selected?.id===id));b.onclick=()=>{$('id-single').checked=false;run({op:'id_select',id});};$('id-found').append(b);}}
  for(const b of $('id-found').children)b.disabled=pending||state.scanning;
  const selected=state.selected;$('id-selected').textContent=selected?`选中 ID ${selected.id} · ${selected.torque?'仍上力':'已断力'} · 原始位置 ${selected.position_raw}`:'尚未选中舵机';
  const stale=feed.age>1500||feed.busy;
  $('id-scan').disabled=pending||state.scanning||stale;$('id-stop').disabled=pending||!state.scanning||stale;
  $('id-relax').disabled=pending||state.scanning||!selected||stale;
  $('id-change').disabled=pending||state.scanning||!selected||selected.torque||!state.found.includes(selected.id)||Boolean(state.error)||!$('id-single').checked||stale;
 }
 async function refresh(){return feed.read();}
 async function run(body){if(pending)return;pending=true;render();try{const r=await command(body);show(r.message);if(body.op==='id_change')$('id-single').checked=false;await refresh();}catch(e){show(e.message,true);}finally{pending=false;render();}}
 $('tab-servo-id').onclick=()=>run({op:'id_enter'});$('tab-calibration').onclick=()=>{if(state?.active)run({op:'id_exit'});else render();};
 $('id-scan').onclick=()=>{$('id-single').checked=false;run({op:'id_scan'});};$('id-stop').onclick=()=>run({op:'id_stop'});
 $('id-relax').onclick=()=>run({op:'id_relax',id:state?.selected?.id});$('id-single').onchange=render;
 $('id-change').onclick=()=>{const sid=state?.selected?.id,new_id=Number($('id-joint').value);if(confirm(`确认只有一只舵机使用 ID ${sid}？其他不同 ID 的舵机可保持连接。将 ID ${sid} 改为 ${new_id}（永久保存），不会修改其他参数。`))run({op:'id_change',id:sid,new_id,single_confirmed:$('id-single').checked});};
 // New-machine settings stay reachable while calibration retains the serial port.
 $('nav-settings').addEventListener('click',event=>{if(!state?.active)return;event.stopImmediatePropagation();event.preventDefault();const u=new URL('/navigate',location.href);u.port=document.querySelector('meta[name="setup-port"]').content;u.search='?target=setup&source=calibration';location.assign(u.href);},true);
 feed.subscribe(next=>{state=next.id_setup;lastSeen=performance.now();if(disconnected){show('连接已恢复，请重新扫描后再写入');disconnected=false;}render();});
 feed.onError(()=>{disconnected=true;show('连接暂不可用，写入已禁用；正在自动重试',true);lastSeen=0;render();});
 document.addEventListener('calibration-command-state',render);
})();
