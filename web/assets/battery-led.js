(() => {
  const enabled = document.getElementById('battery-led-enabled');
  const brightness = document.getElementById('battery-led-brightness');
  const level = document.getElementById('battery-led-level');
  const speed = document.getElementById('battery-led-speed');
  const speedLevel = document.getElementById('battery-led-speed-level');
  const status = document.getElementById('battery-led-status');
  const current = document.getElementById('battery-led-current');
  const endpoint = '/api/battery-led';
  const labels = {
    low_battery_protection: '红灯快闪：低电保护',
    fault: '紫灯反复双闪：持续故障',
    pad_connected: '白灯短闪两下：手柄已连接',
    pad_disconnected: '白灯长亮一下：手柄已断开',
    calibration_pending: '蓝灯慢呼吸：待标定',
    battery_green: '绿灯：电量 ≥ 50%',
    battery_yellow: '黄灯：20% ≤ 电量 < 50%',
    battery_red: '红灯：电量 < 20%',
    battery_unknown: '熄灯：电量未知',
    disabled: '状态灯已关闭',
  };
  let indicatorState;
  let saved;
  let message = '';
  let busy = false;

  function render() {
    enabled.disabled = busy || !saved;
    brightness.disabled = busy || !saved || !enabled.checked;
    speed.disabled = brightness.disabled;
    level.value = brightness.value + '%';
    speedLevel.value = speed.value + ' / 5';
    status.textContent = message ? controlLocale.t(message) : '';
    const label = controlLocale.t(labels[indicatorState] || '状态灯服务未就绪');
    current.textContent = controlLocale.t('当前灯效：{state}', {state: label});
  }

  function apply(settings) {
    enabled.checked = settings.enabled;
    brightness.value = settings.brightness;
    speed.value = settings.speed;
  }

  async function load() {
    try {
      const response = await fetch(endpoint, {signal: AbortSignal.timeout(3000)});
      if (!response.ok) throw new Error();
      const settings = await response.json();
      indicatorState = settings.state;
      if (settings.available) {
        saved = settings;
        apply(saved);
      } else {
        message = '电量灯不可用';
      }
    } catch {
      message = '无法读取电量灯设置';
    }
    render();
  }

  async function save() {
    if (!saved || busy) return;
    busy = true;
    message = '';
    render();
    try {
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({
          enabled: enabled.checked,
          brightness: Number(brightness.value),
          speed: Number(speed.value),
        }),
        signal: AbortSignal.timeout(3000),
      });
      if (!response.ok) throw new Error();
      saved = await response.json();
      message = '电量灯设置已保存';
    } catch {
      message = '无法保存电量灯设置';
    } finally {
      apply(saved);
      busy = false;
      render();
    }
  }

  brightness.addEventListener('input', render);
  brightness.addEventListener('change', save);
  speed.addEventListener('input', render);
  speed.addEventListener('change', save);
  enabled.addEventListener('change', save);
  new MutationObserver(render).observe(document.documentElement, {attributes: true, attributeFilter: ['lang']});
  async function refreshState() {
    if (!document.hidden) {
      try {
        const response = await fetch(endpoint, {signal: AbortSignal.timeout(1500)});
        indicatorState = response.ok ? (await response.json()).state : null;
      } catch {
        indicatorState = null;
      }
      render();
    }
    setTimeout(refreshState, 600);
  }

  load().then(() => setTimeout(refreshState, 600));
})();
