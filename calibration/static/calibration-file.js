/* File parsing only; validation and saving remain in the calibration service. */
'use strict';
window.calibrationFiles={parse(text){
 let profile;
 try{profile=JSON.parse(text);}catch(_){throw Error('不是有效的 JSON 标定文件。');}
 if(!profile||typeof profile!=='object'||Array.isArray(profile)||!profile.imu||
  ['zero','sign','lim'].some(k=>!profile[k]||typeof profile[k]!=='object'||Array.isArray(profile[k])))
  throw Error('标定文件不完整，需要包含舵机 HOME、方向、范围及 IMU 数据。');
 const ids=['10','11','12','13','14','20','21','22','23','24','30','31','32','33','34'];
 if(['zero','sign','lim'].some(k=>Object.keys(profile[k]).sort().join(',')!==ids.join(',')))
  throw Error('标定文件必须对应这套机器人的 15 个舵机。');
 if(new TextEncoder().encode(JSON.stringify(profile)).length>24000)
  throw Error('标定文件内容过大，请选择网页导出的单份标定 JSON。');
 return profile;
}};
