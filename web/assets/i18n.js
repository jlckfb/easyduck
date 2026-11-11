// Operator text for the control page. Diagnostic logs and IDs remain verbatim.
'use strict';
const controlLocale = (() => {
 const english = {
  "Easyduck · 遥控器": "EasyDuck · Remote control",
  "Easyduck 遥控器": "EasyDuck remote control",
  "机器人遥控器": "Robot remote control",
  "页面切换": "Page navigation",
  "连接设置": "Connection settings",
  "标定台": "Calibration",
  "☼ 亮色": "☼ Light",
  "☾ 暗色": "☾ Dark",
  "断力": "Release torque",
  "机器人状态": "Robot status",
  "连接中": "Connecting",
  "等待读取状态": "Waiting for robot status",
  "电量": "Battery",
  "当前控制": "Controller",
  "选择控制端": "Select controller",
  "正在连接机器人…": "Connecting to the robot…",
  "手柄": "Gamepad",
  "手柄优先": "Gamepad priority",
  "用网页控制": "Take web control",
  "交还手柄": "Return to gamepad",
  "等待连接…": "Waiting for connection…",
  "虚拟手柄": "Virtual gamepad",
  "DRIVE / 移动": "DRIVE",
  "双摇杆遥控": "Twin-stick control",
  "速度": "Speed",
  "移动速度比例": "Movement speed",
  "前后 · 转向": "Drive and turn",
  "前后行走、左右转向摇杆": "Drive forward/back and turn left/right",
  "前进": "Forward",
  "后退": "Back",
  "左转": "Turn left",
  "右转": "Turn right",
  "松手即停": "Release to stop",
  "开启行走": "Enable walking",
  "启用行走与倒地恢复": "Enable walking and fall recovery",
  "坐下 / 起立": "Sit / stand",
  "捡物": "Pick up",
  "按住鸣叫": "Hold to call",
  "按得越久，叫声越响": "Hold longer for a louder call",
  "默认站姿": "Default stance",
  "头部 · 上下左右看": "Look around",
  "头部上下左右摇杆": "Look up/down and left/right",
  "抬头": "Look up",
  "低头": "Look down",
  "左看": "Look left",
  "右看": "Look right",
  "颈部俯仰": "Neck pitch",
  "歪头": "Head tilt",
  "头部回中": "Center head",
  "先接管网页，再开启行走。离开窗口时移动自动归零。": "Take web control before enabling walking. Movement returns to zero when the window loses focus.",
  "详细状态": "Detailed status",
  "关节、策略、标定与日志": "Joints, policy, calibration, and logs",
  "状态更新": "Status updates",
  "控制循环": "Control loop",
  "躯干倾角": "Body tilt",
  "当前策略": "Current policy",
  "模型": "Model",
  "角速度": "Angular velocity",
  "重力方向": "Gravity direction",
  "舵机 ID": "Servo ID",
  "当前角度 °": "Current angle °",
  "目标角度 °": "Target angle °",
  "主控日志": "Controller log",
  "摇杆松手停止移动；“断力”解除支撑，请先托稳机器人。动作名称表示主控阶段，实际效果以实物为准。": "Release the stick to stop moving. Support the robot before releasing torque. The action label shows the controller phase. Observe the robot to confirm what it is doing.",
  "请求未完成，请重试": "The request could not be completed. Try again.",
  "已发送断力请求": "Torque release requested",
  "已发送默认站姿请求": "Default stance requested",
  "已发送开启行走请求": "Walking requested",
  "已发送{action}请求": "Requested: {action}",
  "鸣叫": "Calling",
  "回落中": "Fading",
  "机器人状态读取失败，请检查连接": "Could not read robot status. Check the connection.",
  "网页": "Web",
  "等待连接": "Waiting for connection",
  "已连接": "Connected",
  "未连接": "Disconnected",
  "已指定网页": "Web control selected",
  " · 等待控制归零": " · Waiting for neutral input",
  "最近请求：断力": "Last request: release torque",
  "最近请求：默认站姿": "Last request: default stance",
  "最近请求：行走": "Last request: walk",
  "正在查看主控状态": "Reading controller status",
  "未知": "Unknown",
  "就绪": "Ready",
  "未就绪": "Not ready",
  "机器人在线": "Robot online",
  "状态未就绪": "Status unavailable",
  "行走已开启": "Walking enabled",
  "策略已关闭": "Policy off",
  "保持姿态": "Holding position",
  "站姿": "Standing",
  "行走模式": "Walking mode",
  "倒地恢复中": "Recovering from a fall",
  "准备起身": "Preparing to stand",
  "坐姿": "Sitting",
  "起立／站姿保持": "Standing up / holding stance",
  "捡物中": "Picking up",
  "滚翻中": "Rolling over",
  "跳跃中": "Jumping",
  "已断力": "Torque released",
  "其他状态": "Other state",
  "网页已接管，可以操作": "Web control is active",
  "手柄正在控制": "Gamepad is in control",
  "点击「用网页控制」开始": "Select “Take web control” to begin",
  "摇杆松手停止移动；动作按钮按一次执行一次。": "Release the stick to stop moving. Each button press requests one action.",
  "网页未接管，摇杆与动作按钮暂不可用。": "Web control is inactive. Sticks and action buttons are disabled.",
  "原版默认姿态保持 1 秒": "Reference default stance: hold for 1 second",
  "原版倒地恢复": "Reference fall recovery",
  "原版 4 秒捡物周期": "Reference 4-second pickup cycle",
  "请求 {requested} · 应用 {applied} · 限制 {limits}": "Requested {requested} · Applied {applied} · Limits {limits}",
  "无": "None",
  "暂无日志": "No logs yet",
  "已生效（推断）": "Applied (inferred)",
  "已保存，待重启robotd生效": "Saved. Restart robotd to apply.",
  "当前robotd不读网页标定": "This robotd build does not read web calibration",
  "标定状态：{status} · 文件 {file} · 保存于 {saved}（修改 {modified}） · robotd {daemon} 启动于 {started}{error}。robotd不记录已加载哪份标定，“已生效”按文件修改时间早于robotd启动时间推断。": "Calibration: {status} · File {file} · Saved {saved} (modified {modified}) · robotd {daemon} started {started}{error}. The applied status is inferred when the file predates the robotd start time.",
  "连接中断": "Connection lost",
  "等待机器人重新连接": "Waiting for the robot to reconnect",
  "robotd拒绝请求": "robotd rejected the request",
  "控制页面标识无效，请刷新页面": "Invalid control page ID. Reload the page.",
  "需要JSON对象": "A JSON object is required",
  "未知控制入口": "Unknown controller",
  "另一控制页面在线": "Another control page is online",
  "指令字段不正确": "Invalid command fields",
  "网页只支持前后行走和左右转向": "Web control supports forward/back motion and turning",
  "鸣叫强度范围为0～1": "Call intensity must be between 0 and 1",
  "嘴部角度范围为0～30": "Beak angle must be between 0 and 30 degrees",
  "接口不存在": "Endpoint not found",
  "请先在此页面接管控制": "Take control from this page first",
  "当前由手柄控制，或正在等待摇杆归零": "Gamepad is in control or the robot is waiting for neutral input",
  "未知模式": "Unknown mode",
  "未知动作": "Unknown action",
  "当前未加载这个动作的模型": "The model for this action is unavailable",
  "捡物不接受额外参数": "Pickup does not accept extra parameters",
  "只接受当前网页来源的控制请求": "Control requests must come from this page origin",
  "需要application/json": "application/json is required",
  "请求体长度不正确": "Invalid request length",
  "指令维数不正确": "Invalid command dimensions",
  "指令不是有效数值或超出原版网页范围": "Invalid or out-of-range command values",
  "无法确认手柄连接状态，请检查robotctl": "Could not confirm the gamepad connection. Check robotctl.",
  "无法确认手柄连接状态：{detail}": "Could not confirm the gamepad connection: {detail}",
  "robotd通信失败：{detail}；没有自动重发动作": "Could not communicate with robotd: {detail}. The action was not retried.",
  "当前连接已断开，且没有可用的无线地址。请重新连接 USB 或从 Wi-Fi 地址打开网页。": "The connection was lost and no Wi-Fi address is available. Reconnect USB or open the page using the Wi-Fi address.",
  "操作失败，请查看详细状态。": "The operation failed. Check detailed status."
};
 const chineseErrors = {
  "release gamepad sticks, triggers and buttons first": "请先松开手柄摇杆、扳机和按键",
  "web controller is not connected": "网页控制端未连接",
  "control input is inactive or waiting for neutral": "控制端未接管，或正在等待摇杆归零",
  "inactive control input or waiting for neutral": "控制端未接管，或正在等待摇杆归零",
  "low battery: controls locked until robotd restart": "电量过低，控制已锁定，重启 robotd 后解除"
};
 const storageKey = 'easyduck-language';
 const saved = (() => {try{return localStorage.getItem(storageKey);}catch(_){return null;}})();
 const queryLanguage=new URL(location.href).searchParams.get('lang');
 let language = ['zh-CN','en'].includes(queryLanguage) ? queryLanguage : ['zh-CN','en'].includes(saved) ? saved : /^zh(?:-|$)/i.test(globalThis.navigator?.language || 'zh-CN') ? 'zh-CN' : 'en';
 const staticLabels = [];
 function t(source, values = {}){
  const label = language === 'en' && Object.hasOwn(english,source) ? english[source] : source;
  return label.replace(/\{(\w+)\}/g, (token,key) => Object.hasOwn(values,key) ? values[key] : token);
 }
 function error(detail){
  if(language==='zh-CN')return Object.hasOwn(chineseErrors,detail) ? chineseErrors[detail] : detail;
  if(Object.hasOwn(english,detail))return t(detail);
  const transport = /^robotd通信失败：(.*)；没有自动重发动作$/.exec(detail);
  if(transport)return t('robotd通信失败：{detail}；没有自动重发动作',{detail:transport[1]});
  const gamepad = /^无法确认手柄连接状态：(.*)$/.exec(detail);
  if(gamepad)return t('无法确认手柄连接状态：{detail}',{detail:gamepad[1]});
  return /[\u3400-\u9fff]/.test(detail) ? t('操作失败，请查看详细状态。') : detail;
 }
 function apply(){
  document.documentElement.lang=language;
  try{localStorage.setItem(storageKey,language);}catch(_){}
  for(const update of staticLabels)update();
 }
 function init(picker, onChange){
  if(!picker)return;
  const buttons=picker.querySelectorAll('[data-language]');
  // Translate only the original HTML. Live status fields are rendered by app.js.
  const walker=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);
  for(let node=walker.nextNode();node;node=walker.nextNode()){
   const original=node.nodeValue, key=original.trim();
   if(Object.hasOwn(english,key)){
    const target=node;
    staticLabels.push(()=>{target.nodeValue=original.replace(key,t(key));});
   }
  }
  for(const target of document.querySelectorAll('[aria-label]')){
   const key=target.getAttribute('aria-label');
   if(Object.hasOwn(english,key))staticLabels.push(()=>target.setAttribute('aria-label',t(key)));
  }
  const title=document.title;
  staticLabels.push(()=>{document.title=t(title);});
  staticLabels.push(()=>{for(const button of buttons)button.setAttribute('aria-pressed',String(button.dataset.language===language));});
  for(const button of buttons)button.addEventListener('click',()=>{
   if(!['zh-CN','en'].includes(button.dataset.language)||button.dataset.language===language)return;
   language=button.dataset.language;
   const address=new URL(location.href);address.searchParams.set('lang',language);
   history.replaceState(null,'',address);
   try{localStorage.setItem(storageKey,language);}catch(_){}
   apply();
   onChange();
  });
  apply();
 }
 return {t,error,init,get language(){return language;}};
})();
