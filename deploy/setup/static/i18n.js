// Localized labels only; changing language does not submit forms or switch services.
'use strict';
const setupLocale=(()=>{
 const english={
  "Easyduck · 登录": "EasyDuck · Sign in",
  "Easyduck · 连接设置": "EasyDuck · Connection settings",
  "Easyduck · 页面切换": "EasyDuck · Switching pages",
  "Easyduck 首页": "EasyDuck home",
  "☼ 亮色": "☼ Light",
  "☾ 暗色": "☾ Dark",
  "切换暗亮主题": "Switch light/dark theme",
  "本地连接设置": "Local connection settings",
  "退出": "Sign out",
  "设备登录 LOGIN": "DEVICE SIGN IN",
  "连接，从这里开始": "Connect your robot",
  "配置 Wi-Fi 与 PS5 蓝牙手柄": "Set up Wi-Fi and a PS5 Bluetooth gamepad",
  "正在验证系统账号…": "Verifying the system account…",
  "系统账号": "System account",
  "密码": "Password",
  "登录配置中心 ↗": "Sign in ↗",
  "使用该设备的系统账号密码。此页面不设置默认密码。": "Use this device's system account and password. This page does not set a default password.",
  "设备概览 OVERVIEW": "DEVICE OVERVIEW",
  "网络与手柄，连接就绪": "Network and gamepad connections",
  "读取中": "Loading",
  "蓝牙适配器": "Bluetooth adapter",
  "PS5 手柄": "PS5 gamepad",
  "手柄电量": "Gamepad battery",
  "无线访问 ACCESS": "WIRELESS ACCESS",
  "连接 Wi-Fi 后，这里会显示无线控制网页和 SSH 地址。": "Connect to Wi-Fi to see wireless control and SSH addresses here.",
  "打开当前连接的控制页 ↗": "Open control on this connection ↗",
  "通过 Wi-Fi 打开标定页 ↗": "Open calibration over Wi-Fi ↗",
  "仅未编号舵机：分配 ID ↗": "Unnumbered servos only: assign IDs ↗",
  "先确认无线网页可用，再拔掉 USB 数据线。开发板仍需保持供电。": "Check that wireless access works before unplugging USB. Keep the board powered.",
  "连接步骤 QUICK START": "QUICK START",
  "连接 Wi-Fi": "Connect to Wi-Fi",
  "选择网络，输入密码并连接": "Choose a network, enter its password, and connect",
  "配对 PS5 手柄": "Pair a PS5 gamepad",
  "按住 PS + Create，直到灯条闪烁": "Hold PS + Create until the light bar flashes",
  "开始无线使用": "Use wireless control",
  "手机或电脑与机器人处于同一网络": "Connect your phone or computer to the robot's network",
  "WIFI 无线网络": "WI-FI NETWORK",
  "未连接": "Disconnected",
  "已连接": "Connected",
  "读取网络状态": "Reading network status",
  "正在获取网卡与 IP": "Reading Wi-Fi adapter and IP",
  "附近网络": "Nearby networks",
  "正在扫描附近网络…": "Scanning nearby networks…",
  "↻ 扫描网络": "↻ Scan networks",
  "点击“扫描网络”查找附近的 Wi-Fi。": "Select “Scan networks” to find nearby Wi-Fi.",
  "正在连接 Wi-Fi 并获取 IP…": "Connecting to Wi-Fi and obtaining an IP…",
  "Wi-Fi 名称 / SSID": "Wi-Fi name / SSID",
  "从上方选择网络，或输入网络名称": "Choose a network above or enter its name",
  "输入 Wi-Fi 密码，开放网络留空": "Enter the Wi-Fi password; leave blank for open networks",
  "显示密码": "Show password",
  "显示": "Show",
  "隐藏": "Hide",
  "连接成功后自动保存，下次开机自动连接。": "Saved after connecting; reconnects automatically at boot.",
  "PS5 蓝牙手柄": "PS5 BLUETOOTH GAMEPAD",
  "让 DualSense 进入配对模式": "Put DualSense into pairing mode",
  "同时按住": "Hold",
  "和": "and",
  "键，直到灯条快速闪烁，再点击扫描。": "until the light bar flashes rapidly, then scan.",
  "手柄设备": "Gamepads",
  "正在扫描 PS5 手柄，约 8 秒…": "Scanning PS5 gamepads, about 8 seconds…",
  "↻ 扫描手柄": "↻ Scan gamepads",
  "暂未发现手柄，扫描后会显示设备名和 MAC。": "No gamepad found yet. Scan to see device names and MAC addresses.",
  "配对信息会保留，开机后自动尝试重连。手柄休眠或关机时，请先按 PS 键唤醒。": "Pairing is saved and reconnection is attempted at boot. Press PS to wake a sleeping or powered-off gamepad.",
  "刷新连接状态": "Refresh connection status",
  "关闭错误提示": "Dismiss error",
  "请求未完成，请稍后重试。": "The request could not be completed. Try again shortly.",
  "网络无响应。若刚切换 Wi-Fi，请通过原数据连接查看新的 IP 地址。": "No network response. If Wi-Fi just changed, use the original data connection to check the new IP address.",
  "没有发现网络，请开启热点后重新扫描。": "No networks found. Turn on the hotspot and scan again.",
  "信号": "Signal",
  "连接": "Connect",
  "选择网络": "Select network",
  "未连接 Wi-Fi": "Wi-Fi disconnected",
  "未找到网卡": "No Wi-Fi adapter",
  "无 IP": "No IP",
  "正在配对并连接手柄…": "Pairing and connecting the gamepad…",
  "正在连接手柄…": "Connecting the gamepad…",
  "正在断开手柄…": "Disconnecting the gamepad…",
  "正在忘记设备…": "Forgetting the device…",
  "无适配器": "No adapter",
  "已开启": "On",
  "未开启": "Off",
  "暂未发现 PS5 手柄，请进入配对模式后扫描。": "No PS5 gamepad found. Enable pairing mode and scan.",
  "没有蓝牙适配器，请检查蓝牙服务和硬件。": "No Bluetooth adapter. Check the Bluetooth service and hardware.",
  "已配对": "Paired",
  "未配对": "Unpaired",
  "电量：": "Battery: ",
  "自动重连已保存": "Automatic reconnection saved",
  "连接后启用自动重连": "Automatic reconnection enabled after connection",
  "输入设备：": "Input devices: ",
  "输入设备：尚未识别": "Input devices: not detected yet",
  "配对并连接": "Pair and connect",
  "断开": "Disconnect",
  "忘记设备": "Forget device",
  "通过 Wi-Fi 控制机器人 ↗": "Control over Wi-Fi ↗",
  "固定控制地址：http://": "Fixed control address: http://",
  "连接或 IP 改变后会播报地址；策略运行时延后播报。手机不支持固定网址时，使用播报的 IP。": "The address is spoken after connection or IP changes, deferred while a policy is running. Use the spoken IP if your phone cannot resolve the fixed name.",
  "打开标定页 ↗": "Open calibration ↗",
  "打开当前网络的控制页 ↗": "Open control on this network ↗",
  "连接状态已更新。": "Connection status updated.",
  "正在处理…": "Working…",
  "操作等待超时，请先刷新状态，避免重复提交。": "Operation timed out. Refresh status before trying again to avoid duplicate requests.",
  "操作完成。": "Operation completed.",
  "操作失败。": "Operation failed.",
  "EASYDUCK / 页面切换": "EASYDUCK / PAGE HANDOFF",
  "正在准备页面": "Preparing the page",
  "正在检查页面服务。": "Checking page services.",
  "确认断力并进入标定": "Confirm torque release and open calibration",
  "重试": "Retry",
  "返回原页面": "Back to previous page",
  "连接设置": "Connection settings",
  "机器人控制": "Robot control",
  "实物标定台": "Hardware calibration",
  "新机舵机 ID 分配": "New robot servo ID assignment",
  "切换到": "Switch to ",
  "切换失败，请重试。": "Page handoff failed. Try again.",
  "网络连接中断或等待超时，请确认当前无线地址可访问后重试。": "Connection lost or timed out. Check that the current wireless address is reachable before retrying.",
  "正在检查并切换服务，请稍候…": "Checking and switching services. Please wait…",
  "切换等待超时，请检查服务状态后重试。": "Page handoff timed out. Check service status before retrying.",
  "已扶好，断力并切换": "Robot supported: release torque and switch",
  "页面尚未切换，请处理提示后重试。": "The page has not changed. Resolve the reported issue and retry.",
  "请放稳机器人，每次只新增一只待编号舵机，已编号且 ID 不重复的舵机可保留连接。将停止正式控制并进入标定台的 ID 页签，不会上力或修改零点、P/D、限流。": "Place the robot securely. Add only one unnumbered servo at a time; numbered servos with unique IDs may stay connected. Normal control will stop and the servo ID tab will open. This will not enable torque or change zero points, P/D gains, or current limits.",
  "确认并进入 ID 设置": "Confirm and open ID setup",
  "进入标定会停止正式控制并解除全部舵机力矩。请先放稳或托住机器人，给关节留出空间。切换不会上力、执行动作或修改标定。": "Entering calibration stops normal control and releases torque on every servo. Place or support the robot securely and leave room for the joints. Switching will not enable torque, execute motion, or change calibration.",
  "登录尝试过多，请5分钟后重试。": "Too many sign-in attempts. Try again in 5 minutes.",
  "账号或密码错误，或该系统账号不可登录。": "Incorrect account or password, or the system account cannot sign in.",
  "已有连接操作正在执行，请等待完成。": "A connection operation is already running. Wait for it to finish.",
  "连接操作未完成，请检查系统服务后重试。": "The connection operation did not complete. Check system services before retrying.",
  "未知页面。": "Unknown page.",
  "登录已过期，请重新登录。": "Your session expired. Sign in again.",
  "读取连接状态失败，请检查NetworkManager和BlueZ服务。": "Could not read connection status. Check NetworkManager and BlueZ.",
  "操作记录已过期，请刷新状态。": "The operation record expired. Refresh status.",
  "接口不存在。": "Endpoint not found.",
  "请求来源不一致，请从当前页面操作。": "Request origin mismatch. Use the current page.",
  "请求格式无效。": "Invalid request format.",
  "请求校验失败，请重新打开页面。": "Request validation failed. Reopen the page.",
  "请选择列表中的 PS5 手柄。": "Select a PS5 gamepad from the list.",
  "正在处理其他连接操作，请稍后再试。": "Another connection operation is running. Try again shortly.",
  "无法读取 NetworkManager，请检查服务是否运行。": "Could not read NetworkManager. Check that the service is running.",
  "没有可用的 Wi-Fi 网卡，请检查网卡及系统驱动。": "No Wi-Fi adapter available. Check the adapter and system driver.",
  "Wi-Fi 扫描失败，请稍后重试并检查无线开关。": "Wi-Fi scan failed. Check the wireless switch and try again.",
  "Wi-Fi 扫描完成。": "Wi-Fi scan completed.",
  "没有找到该网络，请确认热点已开启、信号可达后重新扫描。": "Network not found. Check that the hotspot is on and in range, then scan again.",
  "已关联目标网络，但没有取得 IP（DHCP 无响应），请检查路由器或热点的 DHCP。": "Associated with the target network but no IP was obtained. Check DHCP on the router or hotspot.",
  "Wi-Fi 密码错误或认证未通过，请重新输入密码。": "Incorrect Wi-Fi password or authentication failed. Enter the password again.",
  "Wi-Fi 认证或连接超时，请核对密码、信号和热点加密方式。": "Wi-Fi authentication or connection timed out. Check the password, signal, and hotspot security.",
  "目标网络未激活或未取得 IP，请核对密码和信号后重试。": "The target network is inactive or has no IP. Check the password and signal, then retry.",
  "SSID 长度应为 1～32 字节。": "The SSID must be 1–32 bytes.",
  "密码包含不支持的字符。": "The password contains unsupported characters.",
  "没有找到该网络，请开启热点后重新扫描。": "Network not found. Turn on the hotspot and scan again.",
  "该网络需要企业认证或旧式 WEP，请使用个人 WPA2/WPA3 或开放网络。": "This network requires enterprise authentication or legacy WEP. Use personal WPA2/WPA3 or an open network.",
  "此网络需要有效的 Wi-Fi 密码（通常为 8～63 字节）。": "Enter a valid Wi-Fi password, usually 8–63 bytes.",
  "NetworkManager 未能载入连接配置。": "NetworkManager could not load the connection profile.",
  "已连接，但自动重连设置未保存，请重试。": "Connected, but automatic reconnection was not saved. Try again.",
  "蓝牙适配器未开启，请检查系统无线开关。": "The Bluetooth adapter is off. Check the system wireless switch.",
  "蓝牙扫描失败，请检查适配器后重试。": "Bluetooth scan failed. Check the adapter and try again.",
  "扫描结束。未出现手柄时，请同时按住 PS 键和 Create 键，直到灯条闪烁。": "Scan finished. If the gamepad is missing, hold PS and Create until the light bar flashes.",
  "未发现该手柄。请同时按住 PS 和 Create 键，灯条闪烁后重新扫描。": "Gamepad not found. Hold PS and Create until the light bar flashes, then scan again.",
  "配对超时，请重新进入配对模式。": "Pairing timed out. Enter pairing mode again.",
  "连接超时，请打开手柄并确认它未连接其他主机。": "Connection timed out. Turn on the gamepad and check that it is not connected to another host.",
  "配对被拒绝或认证失败。请断开其他主机上的手柄连接，再进入配对模式。": "Pairing was rejected or authentication failed. Disconnect the gamepad from other hosts, then enter pairing mode.",
  "设备正在连接或被占用，请断开其他主机连接后重试。": "The device is connecting or in use. Disconnect it from other hosts, then retry.",
  "手柄未响应配对，请按住 PS 和 Create 键直到灯条闪烁；若仍失败，请检查是否已连接其他主机。": "The gamepad did not respond to pairing. Hold PS and Create until the light bar flashes. If it still fails, check other host connections.",
  "手柄连接失败，可能未开启、超出范围或已连接其他主机，请核对后重试。": "Gamepad connection failed. Check that it is on, in range, and disconnected from other hosts.",
  "配对成功，但保存信任状态失败，请重试。": "Paired, but the trusted state was not saved. Try again.",
  "请先配对该手柄。": "Pair this gamepad first.",
  "已连接，但信任状态未保存，请重新配对。": "Connected, but the trusted state was not saved. Pair again.",
  "手柄仍处于连接状态，请重试。": "The gamepad is still connected. Try again.",
  "忘记设备失败，请稍后重试。": "Could not forget the device. Try again shortly.",
  "系统仍保留配对信息，请稍后重试。": "Pairing information still exists. Try again shortly.",
  "不支持的手柄操作。": "Unsupported gamepad operation.",
  "手柄已连接，自动重连已启用。": "Gamepad connected. Automatic reconnection is enabled.",
  "手柄已断开；本次开机期间暂停自动重连，点击连接可恢复。": "Gamepad disconnected. Automatic reconnection is paused until reboot; select Connect to resume.",
  "已忘记该手柄，之后需要重新配对。": "Gamepad forgotten. Pair it again to reconnect.",
  "服务切换超时，请查看服务状态后重试。": "Service handoff timed out. Check service status and retry.",
  "服务切换失败，请检查：": "Service handoff failed. Check: ",
  "页面服务拒绝切换，请刷新状态后重试。": "The page service rejected the handoff. Refresh status and retry.",
  "目标页面服务尚未就绪，请稍后重试。": "The target page service is not ready. Try again shortly.",
  "正式控制正在初始化，请稍候。": "Normal control is initializing. Please wait.",
  "部分已配置舵机未恢复反馈，请检查供电和接线；无需因此重新分配 ID。": "Some configured servos have no feedback. Check power and wiring. Their IDs do not need to be reassigned.",
  "有未保存标定草稿，请先保存或撤销，再切换页面。": "Calibration drafts are unsaved. Save or discard them before switching pages.",
  "实物运动任务尚未结束，请先停止运动。": "A physical motion task is still running. Stop it first.",
  "部分舵机反馈异常，请确认反馈正常后再切换。": "Some servos have abnormal feedback. Restore feedback before switching.",
  "当前舵机仍在上力。退出标定后全部舵机将断力，请扶好机器人，再确认切换。": "Servos are still powered. Leaving calibration will release torque on all servos. Support the robot, then confirm the handoff.",
  "正式控制未确认断力请求，未进入标定。": "Normal control did not confirm torque release. Calibration has not been entered.",
  "其他页面正在切换，请稍后重试。": "Another page handoff is running. Try again shortly.",
  "新机 ID 设置会停止正式控制。请放稳机器人，每次只新增一只待编号舵机；已编号且 ID 不重复的舵机可保留连接后确认。": "Servo ID setup stops normal control. Place the robot securely and add only one unnumbered servo at a time. Numbered servos with unique IDs may remain connected.",
  "请先停止运动并保存或撤销草稿。": "Stop motion and save or discard drafts first.",
  "新机 ID 分配已就绪，不要求15个关节在线。": "Servo ID assignment is ready. All 15 joints do not need to be online.",
  "设置页已就绪，标定服务继续独占串口，正式控制保持停止。": "Settings are ready. Calibration retains exclusive serial access and normal control stays stopped.",
  "正在扫描舵机，请先停止扫描再切换页面。": "Servo scanning is active. Stop scanning before switching pages.",
  "进入标定会断力并停止正式控制，请先放稳机器人后确认。": "Entering calibration releases torque and stops normal control. Support the robot before confirming.",
  "标定页已就绪。": "Calibration is ready.",
  "标定服务退出异常，未恢复正式控制，请检查速度恢复和串口状态。": "Calibration did not exit normally. Normal control has not resumed. Check speed restoration and serial status.",
  "页面已就绪。": "The page is ready.",
  "已连接 {ssid}，IP：{ip}。": "Connected to {ssid}, IP: {ip}.",
  "操作失败，请检查设备状态。": "The operation failed. Check device status."
};
 const supported=value=>value==='en'||value==='zh-CN';
 const key='easyduck-language';
 let stored;try{stored=localStorage.getItem(key);}catch(_){}
 const query=new URL(location.href).searchParams.get('lang');
 let language=supported(query)?query:supported(stored)?stored:/^zh(?:-|$)/i.test(navigator.language||'zh-CN')?'zh-CN':'en';
 const updates=[];
 const reverse=new Map(Object.entries(english).map(([source,label])=>[label,source]));
 function t(source,values={}){
  source=reverse.get(source)||source;
  const label=language==='en'&&Object.hasOwn(english,source)?english[source]:source;
  return label.replace(/\{(\w+)\}/g,(token,name)=>Object.hasOwn(values,name)?values[name]:token);
 }
 function message(source){
  if(Object.hasOwn(english,source)||reverse.has(source))return t(source);
  const connected=/^已连接 (.*)，IP：(.*)。$/.exec(source);
  if(connected)return t('已连接 {ssid}，IP：{ip}。',{ssid:connected[1],ip:connected[2]});
  const servicePrefix='服务切换失败，请检查：';
  if(source.startsWith(servicePrefix))return t(servicePrefix)+source.slice(servicePrefix.length);
  return source;
 }
 function url(href){
  const result=new URL(href,location.href);
  result.searchParams.set('lang',language);
  return result.href;
 }
 function remember(){try{localStorage.setItem(key,language);}catch(_){}}
 function init(onChange){
  const picker=document.getElementById('setup-language');
  const buttons=picker.querySelectorAll('[data-language]');
  const walker=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);
  for(let node=walker.nextNode();node;node=walker.nextNode()){
   const raw=node.nodeValue,source=raw.trim();
   if(Object.hasOwn(english,source)){const target=node;updates.push(()=>{target.nodeValue=raw.replace(source,t(source));});}
  }
  for(const node of document.querySelectorAll('[aria-label],[placeholder],[data-busy]')){
   for(const name of ['aria-label','placeholder','data-busy']){
    const source=node.getAttribute(name);
    if(source&&Object.hasOwn(english,source))updates.push(()=>node.setAttribute(name,t(source)));
   }
  }
  const title=document.title;
  updates.push(()=>{document.title=t(title);});
  function apply(){
   document.documentElement.lang=language;
   for(const button of buttons)button.setAttribute('aria-pressed',String(button.dataset.language===language));
   remember();for(const update of updates)update();
  }
  for(const button of buttons)button.addEventListener('click',()=>{
   if(!supported(button.dataset.language)||button.dataset.language===language)return;
   language=button.dataset.language;
   const address=new URL(location.href);address.searchParams.set('lang',language);
   history.replaceState(null,'',address);
   apply();onChange();
  });
  apply();
 }
 return {t,message,url,init,get language(){return language;}};
})();
