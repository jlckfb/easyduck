'use strict';
const {t}=setupLocale;
const $ = id => document.getElementById(id);
let statusData = null;
let activeOperation = false;
let networkFingerprint = '';
let gamepadFingerprint = '';
const csrf = document.querySelector('meta[name="csrf-token"]')?.content || '';

function setTheme(theme) {
  document.documentElement.dataset.theme = theme === 'day' ? 'day' : 'night';
  if ($('theme-button')) $('theme-button').textContent = theme === 'day' ? t('☾ 暗色') : t('☼ 亮色');
  try { localStorage.setItem('easyduck-theme', theme); } catch (_) {}
}
try { setTheme(localStorage.getItem('easyduck-theme') || 'night'); } catch (_) { setTheme('night'); }
$('theme-button')?.addEventListener('click', () => setTheme(document.documentElement.dataset.theme === 'day' ? 'night' : 'day'));

function toast(message, kind = '', keep = false) {
  const node = document.createElement('div');
  node.className = 'toast ' + kind;
  const label = document.createElement('span');
  label.dataset.message = message;
  label.textContent = setupLocale.message(message);
  node.append(label);
  node.setAttribute('role', kind === 'err' ? 'alert' : 'status');
  $('toasts').append(node);
  if (kind === 'err') {
    const dismiss = document.createElement('button');
    dismiss.type = 'button'; dismiss.className = 'toast-close';
    dismiss.textContent = '×'; dismiss.setAttribute('aria-label', t('关闭错误提示'));
    dismiss.addEventListener('click', () => node.remove()); node.append(dismiss);
    const errors = $('toasts').querySelectorAll('.toast.err');
    if (errors.length > 3) errors[0].remove();
  } else if (!keep) setTimeout(() => node.remove(), 4000);
  return node;
}

async function request(path, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15000);
  try {
    const response = await fetch(path, {credentials:'same-origin', cache:'no-store', ...options, signal:controller.signal});
    const content = await response.json();
    if (!response.ok) throw new Error(content.message || t('请求未完成，请稍后重试。'));
    return content;
  } catch (error) {
    if (error instanceof TypeError || error.name === 'AbortError') {
      throw new Error(t('网络无响应。若刚切换 Wi-Fi，请通过原数据连接查看新的 IP 地址。'));
    }
    throw error;
  } finally { clearTimeout(timeout); }
}

function tag(id, text, kind) {
  const node = $(id);
  if (!node) return;
  node.className = 'tag ' + kind;
  node.textContent = text;
}

function textNode(tagName, text, className = '') {
  const node = document.createElement(tagName);
  node.textContent = text;
  if (className) node.className = className;
  return node;
}

function wifiList(networks) {
  const fingerprint = JSON.stringify(networks);
  if (fingerprint === networkFingerprint) return;
  networkFingerprint = fingerprint;
  const list = $('wifi-networks');
  list.replaceChildren();
  if (!networks.length) list.append(textNode('li', t('没有发现网络，请开启热点后重新扫描。'), 'empty'));
  for (const network of networks) {
    const row = document.createElement('li');
    row.dataset.ssid = network.ssid;
    row.classList.toggle('sel', $('wifi-ssid').value === network.ssid);
    const name = textNode('span', network.ssid, 'nm');
    name.title = network.ssid;
    row.append(name);
    const bars = document.createElement('span');
    bars.className = 'bars';
    bars.setAttribute('aria-label', '信号 ' + network.signal + '%');
    for (let i = 0; i < 4; i++) {
      const bar = document.createElement('i');
      if (i < Math.ceil(network.signal / 25)) bar.className = 'on';
      bars.append(bar);
    }
    row.append(bars, textNode('span', network.signal + '%', 'meta signal-number'), textNode('span', network.security, 'meta'));
    const button = textNode('button', t('连接'), 'mini wifi-pick');
    button.type = 'button';
    button.setAttribute('aria-label', '选择网络 ' + network.ssid);
    button.addEventListener('click', () => {
      $('wifi-ssid').value = network.ssid;
      $('wifi-password').value = '';
      list.querySelectorAll('li').forEach(item => item.classList.toggle('sel', item === row));
      $('wifi-form').scrollIntoView({behavior:'smooth', block:'center'});
      $('wifi-password').focus({preventScroll:true});
      // Selection only. Connection is sent exclusively by the form's submit event.
    });
    row.append(button);
    list.append(row);
  }
}

function applyWifi(wifi) {
  if (!wifi || !$('wifi-name')) return;
  $('wifi-name').firstChild.textContent = wifi.ssid || t('未连接 Wi-Fi');
  $('wifi-detail').textContent = [wifi.device || t('未找到网卡'), wifi.connected ? t('已连接') : t('未连接'), wifi.ip || t('无 IP')].join(' · ');
  tag('wifi-tag', wifi.connected ? t('已连接') : t('未连接'), wifi.connected ? 'ok' : 'off');
  tag('overview-wifi', wifi.connected ? t('已连接') : t('未连接'), wifi.connected ? 'ok' : 'off');
  if (wifi.error) $('wifi-detail').textContent = setupLocale.message(wifi.error);
  wifiList(wifi.networks || []);
}

function deviceAction(action, label, address, danger = false) {
  const form = document.createElement('form');
  form.method = 'post';
  form.action = '/api/bluetooth/' + action;
  form.dataset.busy = {pair:t('正在配对并连接手柄…'),connect:t('正在连接手柄…'),disconnect:t('正在断开手柄…'),forget:t('正在忘记设备…')}[action];
  const input = document.createElement('input');
  input.type = 'hidden'; input.name = 'address'; input.value = address;
  const button = textNode('button', label, 'btn small' + (danger ? ' danger' : ''));
  button.type = 'submit';
  form.append(input, button);
  return form;
}

function applyBluetooth(bluetooth) {
  if (!bluetooth || !$('gamepad-list')) return;
  const adapter = bluetooth.adapter || {};
  const state = !adapter.available ? t('无适配器') : adapter.powered ? t('已开启') : t('未开启');
  tag('adapter-tag', state, adapter.available && adapter.powered ? 'ok' : 'off');
  tag('overview-adapter', state, adapter.available && adapter.powered ? 'ok' : 'off');
  const devices = bluetooth.devices || [];
  const connected = devices.find(d => d.connected);
  tag('overview-pad', connected ? t('已连接') : t('未连接'), connected ? 'ok' : 'off');
  const batteryKnown = value => Number.isFinite(value) && value >= 0 && value <= 100;
  const overviewBatteryKnown = batteryKnown(connected?.battery);
  $('overview-battery-row').hidden = !overviewBatteryKnown;
  $('overview-battery').textContent = overviewBatteryKnown ? connected.battery + '%' : '';
  const fingerprint = JSON.stringify(devices);
  if (fingerprint === gamepadFingerprint) return;
  gamepadFingerprint = fingerprint;
  const list = $('gamepad-list');
  list.replaceChildren();
  if (!devices.length) list.append(textNode('div', adapter.available ? t('暂未发现 PS5 手柄，请进入配对模式后扫描。') : t('没有蓝牙适配器，请检查蓝牙服务和硬件。'), 'empty'));
  for (const device of devices) {
    const article = document.createElement('article'); article.className = 'gamepad'; article.dataset.mac = device.address;
    const head = document.createElement('div'); head.className = 'gamepad-head';
    const title = document.createElement('div');
    title.append(textNode('h3', device.name, 'gamepad-name'), textNode('span', device.address, 'mac'));
    head.append(title, textNode('span', device.connected ? t('已连接') : t('未连接'), 'tag ' + (device.connected ? 'ok' : 'off')));
    const meta = document.createElement('div'); meta.className = 'gamepad-meta';
    meta.append(textNode('span', device.paired ? t('已配对') : t('未配对')));
    if (batteryKnown(device.battery)) meta.append(textNode('span', t('电量：') + device.battery + '%'));
    meta.append(textNode('span', device.auto_reconnect ? t('自动重连已保存') : t('连接后启用自动重连')));
    const input = textNode('p', device.inputs?.length ? t('输入设备：') + device.inputs.join(' · ') : t('输入设备：尚未识别'), 'note');
    const actions = document.createElement('div'); actions.className = 'gamepad-actions';
    if (!device.paired) actions.append(deviceAction('pair', t('配对并连接'), device.address));
    else if (!device.connected) actions.append(deviceAction('connect', t('连接'), device.address));
    if (device.connected) actions.append(deviceAction('disconnect', t('断开'), device.address));
    if (device.paired) actions.append(deviceAction('forget', t('忘记设备'), device.address, true));
    article.append(head, meta, input, actions); list.append(article);
  }
}

function applyAccess(data) {
  const wifi = data.wifi || {};
  const access = $('access-list');
  access.replaceChildren();
  const controlTarget = new URL(setupLocale.url('/navigate?target=control&source=setup'));
  const wirelessReady = wifi.connected && /^\d{1,3}(?:\.\d{1,3}){3}$/.test(wifi.ip);
  if (wirelessReady) {
    const wifiTarget = new URL(controlTarget.href);wifiTarget.hostname = wifi.ip;
    const link = textNode('a', t('通过 Wi-Fi 控制机器人 ↗'), 'access-link'); link.href = wifiTarget.href;
    const settings = textNode('a', 'http://' + wifi.ip + (location.port && location.port !== '80' ? ':' + location.port : '') + '/', 'access-link'); settings.href = settings.textContent;
    access.append(link, settings, textNode('div', 'ssh ' + data.login_user + '@' + wifi.ip, 'access-address'));
  } else access.append(textNode('p', t('连接 Wi-Fi 后，这里会显示无线控制网页和 SSH 地址。'), 'note'));
  const localName = data.network_access?.hostname;
  if (wirelessReady && typeof localName === 'string' && /^[a-zA-Z0-9-]+\.local$/.test(localName)) {
    const fixed = textNode('a', t('固定控制地址：http://') + localName + ':9527/', 'access-link');
    fixed.href = 'http://' + localName + ':9527/'; access.append(fixed);
    access.append(textNode('p', t('连接或 IP 改变后会播报地址；策略运行时延后播报。手机不支持固定网址时，使用播报的 IP。'), 'note'));
  }
  const control = $('current-control'); control.href = controlTarget.href; control.hidden = false;
  const calibration = $('open-calibration');
  const calibrationTarget = new URL(controlTarget.href); calibrationTarget.searchParams.set('target','calibration');
  calibration.href = calibrationTarget.href; calibration.hidden = false;
  const idTarget=new URL(calibrationTarget.href);idTarget.searchParams.set('target','servo_setup');$('open-servo-id').href=idTarget.href;$('open-servo-id').hidden=false;
  calibration.textContent = t('打开标定页 ↗');
  control.textContent = t('打开当前网络的控制页 ↗');
}

async function refreshStatus(showToast = false) {
  const response = await request('/api/status');
  statusData = response.data;
  if ($('hostname')) {
    $('hostname').firstChild.textContent = statusData.hostname || 'Easyduck';
    applyWifi(statusData.wifi); applyBluetooth(statusData.bluetooth); applyAccess(statusData);
  }
  if (showToast) toast(t('连接状态已更新。'), 'ok');
}

const APPLY = {
  '/api/wifi/scan': result => applyWifi(result.wifi),
  '/api/wifi/connect': result => { applyWifi(result.wifi); if (result.ok) $('wifi-password').value = ''; },
  '/api/bluetooth/scan': result => applyBluetooth(result.bluetooth),
  '/api/bluetooth/pair': result => applyBluetooth(result.bluetooth),
  '/api/bluetooth/connect': result => applyBluetooth(result.bluetooth),
  '/api/bluetooth/disconnect': result => applyBluetooth(result.bluetooth),
  '/api/bluetooth/forget': result => applyBluetooth(result.bluetooth)
};

function disableActions(disabled) {
  document.querySelectorAll('form button[type=submit]').forEach(button => { button.disabled = disabled; });
}

document.addEventListener('submit', async event => {
  const form = event.target;
  if (!(form instanceof HTMLFormElement) || !new URL(form.action).pathname.startsWith('/api/')) return;
  event.preventDefault();
  if (activeOperation) return;
  activeOperation = true; disableActions(true);
  const path = new URL(form.action).pathname;
  const progress = toast(form.dataset.busy || t('正在处理…'), 'busy', true);
  const body = Object.fromEntries(new FormData(form).entries());
  try {
    let result = await request(path, {method:'POST', headers:{'Content-Type':'application/json','X-CSRF-Token':csrf}, body:JSON.stringify(body)});
    if (path === '/api/login') { const query=new URL(location.href).searchParams; const target=query.get('target'); const source=query.get('source'); location.assign(setupLocale.url(['setup','control','calibration','servo_setup'].includes(target)?'/navigate?target='+target+'&source='+(['setup','control','calibration'].includes(source)?source:'setup'):'/')); return; }
    if (path === '/api/logout') { location.assign(setupLocale.url('/login')); return; }
    if (result.job) {
      const deadline = Date.now() + 95000;
      for (;;) {
        if (Date.now() > deadline) throw new Error(t('操作等待超时，请先刷新状态，避免重复提交。'));
        await new Promise(resolve => setTimeout(resolve, 500));
        const job = await request('/api/jobs/' + result.job);
        if (job.done) { result = job.result; break; }
      }
    }
    APPLY[path]?.(result);
    toast(result.message || (result.ok ? t('操作完成。') : t('操作失败。')), result.ok ? 'ok' : 'err');
    await refreshStatus(false).catch(() => {});
  } catch (error) { toast(error.message, 'err'); }
  finally { progress.remove(); activeOperation = false; disableActions(false); }
});

$('show-password')?.addEventListener('click', () => {
  const input = $('wifi-password'); input.type = input.type === 'password' ? 'text' : 'password';
  $('show-password').textContent = input.type === 'password' ? t('显示') : t('隐藏');
});
$('refresh-status')?.addEventListener('click', () => refreshStatus(true).catch(error => toast(error.message, 'err')));
setupLocale.init(()=>{
  setTheme(document.documentElement.dataset.theme);
  const password=$('wifi-password');
  if(password)$('show-password').textContent=password.type==='password'?t('显示'):t('隐藏');
  networkFingerprint=gamepadFingerprint='';
  if(statusData){applyWifi(statusData.wifi);applyBluetooth(statusData.bluetooth);applyAccess(statusData);}
  for(const node of document.querySelectorAll('[data-message]'))node.textContent=setupLocale.message(node.dataset.message);
});
if ($('wifi-card')) {
  refreshStatus().catch(error => toast(error.message, 'err'));
  setInterval(() => { if (!document.hidden && !activeOperation) refreshStatus().catch(() => {}); }, 6000);
}
