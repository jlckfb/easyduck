(() => {
  const button = document.getElementById('camera-toggle');
  const image = document.getElementById('camera-preview');
  const status = document.getElementById('camera-status');
  const {t} = controlLocale;
  let active = false;
  let state = '';

  function render() {
    button.textContent = t(active ? '关闭摄像头' : '开启摄像头');
    button.setAttribute('aria-pressed', String(active));
    status.textContent = state ? t(state) : '';
    image.alt = t('USB 摄像头画面');
  }
  function stop(message = '') {
    active = false;
    image.hidden = true;
    image.removeAttribute('src');
    state = message;
    render();
  }

  button.addEventListener('click', () => {
    if (active) return stop();
    active = true;
    state = '正在连接摄像头…';
    image.src = '/api/camera/stream';
    render();
  });
  image.addEventListener('load', () => {
    if (!active) return;
    image.hidden = false;
    state = '';
    render();
  });
  image.addEventListener('error', () => {
    if (active) stop('摄像头不可用，请检查 USB 连接或关闭其他预览页。');
  });
  window.addEventListener('pagehide', () => stop());
  new MutationObserver(render).observe(document.documentElement, {attributes: true, attributeFilter: ['lang']});
  render();
})();
