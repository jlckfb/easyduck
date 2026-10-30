/* Read-only connection checks; switching services still requires the normal handoff. */
'use strict';
(() => {
 let wifiHost='';
 const valid=host=>typeof host==='string'&&/^(?:\d{1,3}\.){3}\d{1,3}$/.test(host)&&host.split('.').every(v=>Number(v)<=255);
 async function refresh(){
  const response=await fetch('/api/network-addresses',{cache:'no-store',signal:AbortSignal.timeout(1500)});
  if(!response.ok)throw Error('无法读取网络地址');
  const data=await response.json();wifiHost=valid(data.wifi_host)?data.wifi_host:'';
 }
 let initial=refresh().catch(()=>{});
 setInterval(()=>refresh().catch(()=>{}),5000);
 window.easyduckNavigation=async href=>{
  const url=new URL(href,location.href);
  await initial;initial=Promise.resolve();
  try{await refresh();return url.href;}catch(_){
   if(wifiHost&&wifiHost!==location.hostname){url.hostname=wifiHost;return url.href;}
   throw Error('当前连接已断开，且没有可用的无线地址。请重新连接 USB 或从 Wi-Fi 地址打开网页。');
  }
 };
 document.addEventListener('click',async event=>{
  const link=event.target.closest?.('a[href]');
  if(event.defaultPrevented||!link||event.button!==0||event.ctrlKey||event.metaKey||event.shiftKey||event.altKey)return;
  const url=new URL(link.href);
  if(url.pathname!=='/navigate'||url.hostname!==location.hostname)return;
  event.preventDefault();
  try{location.assign(await window.easyduckNavigation(url.href));}catch(error){alert(error.message);}
 });
})();
