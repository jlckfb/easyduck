// One state request for the entire page; hardware commands are never retried.
(() => {
 let latest=null,inflight=null,busy=false,lastSeen=0;
 const subscribers=new Set(),errors=new Set();
 const emit=()=>{for(const fn of subscribers){try{fn(latest);}catch(e){console.error('Calibration view:',e);document.dispatchEvent(new CustomEvent('calibration-view-error',{detail:e.message}));}}};
 async function read(){
  if(busy)return latest;
  if(inflight)return inflight;
  inflight=(async()=>{
   const r=await fetch('/api/state',{signal:AbortSignal.timeout(5000)});
   if(!r.ok)throw Error('状态读取失败（HTTP '+r.status+'）');
   const next=await r.json();latest=next;lastSeen=performance.now();emit();return next;
  })();
  try{return await inflight;}finally{inflight=null;}
 }
 async function command(body){
  if(busy)throw Error('上一项操作尚未完成，请稍候');
  busy=true;document.dispatchEvent(new Event('calibration-command-state'));
  try{
   if(inflight)await inflight.catch(()=>{});
   const r=await fetch('/api/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(15000)});
   const result=await r.json();if(!r.ok)throw Error(result.error||'操作失败');return result;
  }catch(e){
   if(e.name==='TimeoutError'||e.name==='TypeError')throw Error('未收到操作确认：请核对实时状态后再操作，不要重复点击写入');
   throw e;
  }finally{busy=false;document.dispatchEvent(new Event('calibration-command-state'));}
 }
 window.calibrationFeed={read,command,subscribe(fn){subscribers.add(fn);if(latest)fn(latest);},onError(fn){errors.add(fn);},get busy(){return busy;},get age(){return lastSeen?performance.now()-lastSeen:Infinity;}};
 async function poll(){try{await read();}catch(e){lastSeen=0;for(const fn of errors)fn(e);}setTimeout(poll,200);}
 poll();
})();
