const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict'),path=require('node:path');
const html=fs.readFileSync(path.join(__dirname,'../assets/index.html'),'utf8');
assert(!/id="(?:jump|roulade)"/.test(html));
const elements=new Map();
function element(id){return {id,value:['head0','head3'].includes(id)?'0':id==='speed'?'50':'',disabled:false,dataset:{},textContent:'',tagName:'DIV',style:{transform:'',setProperty(){}},classList:{toggle(){},add(){},remove(){}},setAttribute(k,v){this[k]=v;},clientWidth:200,offsetWidth:60,firstElementChild:{offsetWidth:60,style:{}},hasPointerCapture(){return this.capture!=null;},setPointerCapture(p){this.capture=p;},releasePointerCapture(){this.capture=null;},getBoundingClientRect(){return {left:0,top:0,width:200,height:200};},querySelector(){return this.label||(this.label={textContent:'action'});}};}
for(const m of html.matchAll(/id="([^"]+)"/g))elements.set(m[1],element(m[1]));
elements.delete('control-language');
const $=id=>{if(id==='control-language')return null;assert(elements.has(id),'unknown element '+id);return elements.get(id);};
const intervals=[],requests=[],events={};let now=0;
const drive=element('drive'),sitButton=element('sit'),pickButton=element('pick');sitButton.dataset={skill:'sit_toggle',model:'posture'};pickButton.dataset={skill:'ground_pick',model:'ground_pick'};let statusFixture={};
const ctx={document:{hidden:false,documentElement:{dataset:{}},getElementById:$,querySelector(s){return s.startsWith('meta')?{content:'9527'}:drive;},querySelectorAll(s){return s==='#head-controls input'?[$('head0'),$('head3')]:s==='[data-skill]'?[sitButton,pickButton]:[];},addEventListener(n,f){events[n]=f;}},window:{addEventListener(n,f){events[n]=f;}},performance:{now:()=>now},setInterval(f,ms){intervals.push({f,ms});},setTimeout(){},localStorage:{getItem(){return 'night';},setItem(){}},location:{protocol:'http:',host:'mock',href:'http://mock/'},URL,AbortSignal,console,crypto:{randomUUID:()=> 'test'},WebSocket:class{close(){}},fetch:async(url,args)=>{requests.push({url,body:args?.body&&JSON.parse(args.body)});return {ok:true,json:async()=>statusFixture};}};
vm.createContext(ctx);vm.runInContext(fs.readFileSync(path.join(__dirname,'../assets/i18n.js'),'utf8'),ctx);vm.runInContext(fs.readFileSync(path.join(__dirname,'../assets/app.js'),'utf8'),ctx);
const run=code=>vm.runInContext(code,ctx);const event=(x,y)=>({clientX:x,clientY:y,pointerId:1,button:0,preventDefault(){}});
(async()=>{
 run('webAllowed=true');$('joy-left').onpointerdown(event(30,30));assert.equal(run('motion[1]'),0);assert(run('motion[0]')>0&&run('motion[2]')>0);
 $('joy-left').onpointerup(event(30,30));assert.deepEqual(Array.from(run('motion')),[0,0,0]);
 $('joy-right').onpointerdown(event(150,50));await new Promise(setImmediate);const cmd=requests.filter(r=>r.url==='/api/command').at(-1).body;
 assert.equal(cmd.twist[2],0);assert(cmd.head[1]<0&&cmd.head[2]<0);assert.equal(cmd.head[0],0);
 $('head0').value='.3';$('head3').value='-.2';$('head3').oninput();await new Promise(setImmediate);
 const adjusted=requests.filter(r=>r.url==='/api/command').at(-1).body;assert.equal(adjusted.head[0],.3);assert.equal(adjusted.head[3],-.2);
 $('head-zero').onclick();await new Promise(setImmediate);assert.deepEqual(requests.filter(r=>r.url==='/api/command').at(-1).body.head,[0,0,0,0]);
 $('call').onpointerdown(event(0,0));const tick=intervals.find(i=>i.ms===50).f;
 for(let i=0;i<3;i++){now+=50;tick();}assert(Math.abs(run('callLevel')-3/7)<1e-6);
 for(let i=0;i<5;i++){now+=50;tick();}assert.equal(run('callLevel'),1);
 $('call').onpointerup(event(0,0));await new Promise(setImmediate);const released=requests.filter(r=>r.url==='/api/command').at(-1).body;assert.equal(released.vocal,0);assert.equal(released.mouth,0);assert.equal(run('callLevel'),0);now+=50;tick();assert.equal(run('callLevel'),0);
 for(let i=0;i<3;i++){$('call').onpointerdown(event(0,0));now+=50;tick();assert(run('callLevel')>0);$('call').onpointerup(event(0,0));await new Promise(setImmediate);const tap=requests.filter(r=>r.url==='/api/command').at(-1).body;assert.equal(tap.mouth,0);assert.equal(tap.vocal,0);}
 for(let i=0;i<10;i++){now+=50;tick();}assert.equal(run('callLevel'),0);
 $('call').onpointerdown(event(0,0));now+=50;tick();events.blur();assert.equal(run('callLevel'),0);assert.equal(run('callHeld'),false);
 run('webAllowed=false');$('call').onpointerdown(event(0,0));assert.equal(run('callHeld'),false);
 const poll=intervals.find(i=>i.ms===250).f;
 for(const phase of ['xgo_walk','xgo_sit','xgo_rise','xgo_pick','xgo_getup','xgo_hold']){
  statusFixture={feedback_age_ms:5,this_page:true,enabled:true,control:{owner:'web',ready:true,generation:null},state:{policy:phase},models:{posture:'posture',ground_pick:'pick'},motors:[]};
  await poll();assert.equal(drive.disabled,true,phase+' must not offer to re-enable an enabled policy');
  assert.equal(drive.querySelector('b').textContent,'行走已开启');
  if(phase==='xgo_rise'){assert.equal(sitButton.disabled,false,'standing posture can sit again without enabling walking');assert.equal(pickButton.disabled,false,'standing posture can pick without first moving');}
  if(['xgo_sit','xgo_getup'].includes(phase))assert.equal(pickButton.disabled,true);
 }
 const start=requests.length;$('speed').value='80';$('speed').oninput();await new Promise(setImmediate);assert.equal(drive.disabled,true);assert(requests.slice(start).every(r=>r.url==='/api/command'));
 statusFixture={...statusFixture,enabled:undefined,state:{policy:'xgo_rise'}};await poll();assert.equal(drive.disabled,true,'old daemon posture is still active');
 statusFixture={...statusFixture,enabled:false,state:{policy:'xgo_off'}};await poll();assert.equal(drive.disabled,false);assert.equal(drive.querySelector('b').textContent,'开启行走');
 console.log('PASS: walking/turning, independent head axes, recenter, vocal rise/release, blur and ownership; mocked network only');
})().catch(e=>{console.error(e);process.exitCode=1;});
