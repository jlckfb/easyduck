(() => {
 let theme='night';try{theme=localStorage.getItem('easyduck-theme')==='day'?'day':'night';}catch(_){}
 function apply(){document.documentElement.dataset.theme=theme;const b=document.getElementById('themeToggle');if(b){b.textContent=theme==='day'?'☾ 深色':'☼ 浅色';b.setAttribute('aria-label',theme==='day'?'切换深色主题':'切换浅色主题');}}
 apply();
 document.getElementById('themeToggle').onclick=()=>{theme=theme==='day'?'night':'day';try{localStorage.setItem('easyduck-theme',theme);}catch(_){}apply();};
 window.calibrationFeed.subscribe(s=>{document.getElementById('railMode').textContent=s.mock?'离线演示':'实物连接';document.getElementById('railOnline').textContent=s.rows.filter(r=>r.count!=null).length+' / 15';});
})();
