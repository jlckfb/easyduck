(() => {
  const form = document.getElementById('intercom-login');
  const status = document.getElementById('intercom-status');
  const secureLink = document.getElementById('intercom-https');
  const talk = document.getElementById('intercom-talk');
  const close = document.getElementById('intercom-close');
  const output = document.getElementById('intercom-output');
  const outputStatus = document.getElementById('intercom-output-status');
  const open = document.getElementById('intercom-open');
  const username = document.getElementById('intercom-user');
  const password = document.getElementById('intercom-password');
  const prefix = '/api/intercom';
  const errors = {
    intercom_busy: '另一台手机正在对讲',
    intercom_retry: '请稍后再试',
    intercom_login_failed: '连接设置的账号或密码不正确',
    intercom_audio_failed: '鸭子的音频设备不可用',
  };
  let audio, mic, micSource, worklet, token, socket, heartbeat;
  let streamEnabled = false;
  let opening = false;
  let epoch = 0;
  let holding = false;
  let activeTouch = null;
  let micRequest = 0;
  let flushUntil = 0;
  let nextPlayback = 0;
  let message = '';
  let outputMessage = '';
  let selectedOutput = '';
  const outgoing = [];
  const playing = new Set();

  function show(text) {
    message = text;
    status.textContent = controlLocale.t(text);
  }

  function showOutput(text, device = '') {
    outputMessage = text;
    selectedOutput = device;
    outputStatus.textContent = controlLocale.t(text, {device});
  }

  async function chooseOutput() {
    const active = audio;
    const devices = navigator.mediaDevices;
    const unavailable = '浏览器没有提供可切换的扬声器，请在手机系统里选择音频输出。';
    if (!token || typeof active?.setSinkId !== 'function') {
      showOutput(unavailable);
      return;
    }
    output.disabled = true;
    try {
      const device = typeof devices.selectAudioOutput === 'function'
        ? await devices.selectAudioOutput()
        : (await devices.enumerateDevices()).find(item => item.kind === 'audiooutput'
          && item.deviceId && item.deviceId !== 'default'
          && /^(?:built-in (?:phone )?|phone |internal )?(?:loud)?speakers?$|^(?:内置|手机)?(?:扬声器|外放)$/i.test(item.label.trim()));
      if (active !== audio) return;
      if (!device) {
        showOutput(unavailable);
        return;
      }
      await active.setSinkId(device.deviceId);
      if (active === audio) showOutput('已选音频输出：{device}', device.label);
    } catch (_) {
      if (active === audio) showOutput('未能切换扬声器，请检查系统音频输出。');
    } finally {
      if (active === audio) output.disabled = false;
    }
  }

  function stopPlayback() {
    for (const src of playing) src.stop();
    playing.clear();
    nextPlayback = 0;
  }

  function play(pcm) {
    if (!pcm.byteLength || holding || flushUntil || audio.state !== 'running') return;
    const buffer = audio.createBuffer(1, pcm.byteLength / 2, 16000);
    const data = buffer.getChannelData(0);
    const view = new DataView(pcm);
    for (let i = 0; i < data.length; i++) data[i] = view.getInt16(i * 2, true) / 32768;
    const now = audio.currentTime;
    if (nextPlayback > now + (socket ? 1.6 : 0.25)) stopPlayback();
    const src = audio.createBufferSource();
    src.buffer = buffer;
    src.connect(audio.destination);
    src.onended = () => playing.delete(src);
    playing.add(src);
    const start = nextPlayback > now ? nextPlayback : now + (socket ? 0.65 : 0.065);
    src.start(start);
    nextPlayback = start + buffer.duration;
  }

  function exchange(mode, data, session = token) {
    return fetch(prefix + '/' + mode, {
      method: 'POST',
      headers: {'Content-Type': 'application/octet-stream', 'X-Intercom-Token': session},
      body: data || new Uint8Array(),
      signal: mode === 'close' ? undefined : AbortSignal.timeout(1800),
      keepalive: mode === 'close',
    });
  }

  function connectStream(session) {
    const address = new URL(prefix + '/stream', location.href);
    address.protocol = 'wss:';
    const active = new WebSocket(address);
    active.binaryType = 'arraybuffer';
    socket = active;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('stream timeout')), 3000);
      active.onopen = () => active.send(session);
      active.onmessage = ({data}) => {
        if (socket !== active) return;
        if (data === 'ready') {
          clearTimeout(timeout);
          heartbeat = setInterval(() => {
            if (active.readyState === WebSocket.OPEN) active.send('ping');
          }, 1000);
          resolve();
        } else if (data instanceof ArrayBuffer) {
          play(data);
        }
      };
      active.onclose = () => {
        clearTimeout(timeout);
        reject(new Error('stream closed'));
        if (socket === active && !opening) disconnect('对讲连接已断开，请重新进入');
      };
    });
  }

  function requestMicrophone() {
    return navigator.mediaDevices.getUserMedia({
      audio: {channelCount: 1, noiseSuppression: true, echoCancellation: true}
    });
  }

  function stopMicrophone() {
    micSource?.disconnect();
    micSource = null;
    mic?.getTracks().forEach(track => track.stop());
    mic = null;
  }

  function disconnect(text = '对讲已关闭') {
    epoch++;
    const old = token;
    token = null;
    const active = socket;
    socket = null;
    clearInterval(heartbeat);
    if (active) active.close();
    if (old) exchange('close', null, old).catch(() => {});
    holding = false;
    activeTouch = null;
    flushUntil = 0;
    outgoing.length = 0;
    talk.classList.remove('intercom-speaking');
    stopPlayback();
    micRequest++;
    stopMicrophone();
    if (audio && audio.state !== 'closed') audio.close().catch(() => {});
    mic = audio = worklet = null;
    talk.disabled = true;
    output.disabled = true;
    showOutput('');
    close.hidden = true;
    open.disabled = false;
    form.hidden = false;
    show(text);
  }

  async function run(session) {
    while (token === session) {
      await new Promise(resolve => setTimeout(resolve, holding || flushUntil ? 15 : 0));
      if (token !== session) return;
      if (flushUntil && performance.now() > flushUntil) flushUntil = 0;
      const pcm = outgoing.shift();
      const transmitting = holding || flushUntil || pcm;
      if (transmitting && !pcm) continue;
      const reply = await exchange(transmitting ? 'talk' : 'listen', pcm, session);
      if (token !== session) return;
      if (!reply.ok) {
        const {detail} = await reply.json();
        throw new Error(detail);
      }
      const response = await reply.arrayBuffer();
      if (token === session && !transmitting) play(response);
    }
  }

  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (opening || token) return;
    opening = true;
    open.disabled = true;
    const current = ++epoch;
    show('正在连接对讲');
    try {
      audio = new AudioContext();
      await audio.resume();
      const permissionStream = await requestMicrophone();
      permissionStream.getTracks().forEach(track => track.stop());
      if (current !== epoch) return;
      await audio.audioWorklet.addModule('/intercom-worklet.js?v=20261120');
      if (current !== epoch) return;
      worklet = new AudioWorkletNode(audio, 'intercom-capture');
      worklet.port.onmessage = ({data}) => {
        if (data === 'flushed') {
          flushUntil = 0;
          if (socket?.readyState === WebSocket.OPEN) socket.send('listen');
        } else if (holding || flushUntil) {
          if (socket) {
            if (socket.readyState === WebSocket.OPEN && socket.bufferedAmount < 11520) socket.send(data);
            return;
          }
          if (outgoing.length >= 3) outgoing.shift();
          outgoing.push(data);
        }
      };
      const muted = audio.createGain();
      muted.gain.value = 0;
      worklet.connect(muted).connect(audio.destination);
      const login = fetch(prefix + '/open', {
        method: 'POST', headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({username: username.value, password: password.value}),
        signal: AbortSignal.timeout(3500),
      });
      password.value = '';
      const reply = await login;
      const data = await reply.json();
      if (!reply.ok) throw new Error(data.detail);
      if (current !== epoch) {
        exchange('close', null, data.token).catch(() => {});
        return;
      }
      token = data.token;
      if (streamEnabled) await connectStream(token);
      if (current !== epoch) return;
      form.hidden = true;
      close.hidden = false;
      talk.disabled = false;
      output.disabled = false;
      show('正在收听鸭子周围的声音');
      const session = token;
      if (!streamEnabled) run(session).catch(error => {
        if (token === session) disconnect(errors[error.message] || '对讲连接已断开，请重新进入');
      });
    } catch (error) {
      if (current === epoch) {
        disconnect(errors[error.message] ||
          (error.name === 'NotAllowedError' ? '请允许浏览器使用手机麦克风' : '对讲连接失败'));
      }
    } finally {
      password.value = '';
      opening = false;
      open.disabled = false;
    }
  });

  async function startTalk() {
    if (!token || holding || flushUntil) return;
    stopPlayback();
    outgoing.length = 0;
    holding = true;
    const request = ++micRequest;
    show('正在打开手机麦克风');
    try {
      const stream = await requestMicrophone();
      if (!holding || request !== micRequest) {
        stream.getTracks().forEach(track => track.stop());
        return;
      }
      mic = stream;
      micSource = audio.createMediaStreamSource(mic);
      micSource.connect(worklet);
      if (socket?.readyState === WebSocket.OPEN) socket.send('talk');
      worklet.port.postMessage('start');
      talk.classList.add('intercom-speaking');
      show('正在说话，松开收听');
    } catch (_) {
      if (request !== micRequest) return;
      releaseTalk();
      show('请允许浏览器使用手机麦克风');
    }
  }

  function releaseTalk() {
    activeTouch = null;
    if (!holding) return;
    holding = false;
    micRequest++;
    if (mic) {
      flushUntil = performance.now() + 180;
      worklet.port.postMessage('finish');
      stopMicrophone();
    }
    talk.classList.remove('intercom-speaking');
    show('正在收听鸭子周围的声音');
  }

  talk.addEventListener('touchstart', event => {
    if (event.cancelable) event.preventDefault();
    if (!token || holding || flushUntil || event.changedTouches.length !== 1) return;
    activeTouch = event.changedTouches[0].identifier;
    startTalk();
  }, {passive: false});
  talk.addEventListener('touchmove', event => {
    if (activeTouch !== null && event.cancelable) event.preventDefault();
  }, {passive: false});
  for (const type of ['touchend', 'touchcancel']) {
    talk.addEventListener(type, event => {
      if (!Array.from(event.changedTouches).some(touch => touch.identifier === activeTouch)) return;
      if (event.cancelable) event.preventDefault();
      releaseTalk();
    }, {passive: false});
  }
  talk.addEventListener('pointerdown', event => {
    if (event.pointerType === 'touch' || !event.isPrimary || event.button !== 0) return;
    event.preventDefault();
    talk.setPointerCapture(event.pointerId);
    startTalk();
  });
  for (const event of ['pointerup', 'pointercancel', 'lostpointercapture']) {
    talk.addEventListener(event, pointer => {
      if (pointer.pointerType !== 'touch') releaseTalk();
    });
  }
  talk.addEventListener('contextmenu', event => event.preventDefault());
  talk.addEventListener('keydown', event => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    startTalk();
  });
  talk.addEventListener('keyup', event => {
    if (event.key === 'Enter') releaseTalk();
  });
  talk.addEventListener('blur', releaseTalk);
  close.addEventListener('click', () => disconnect());
  output.addEventListener('click', chooseOutput);
  window.addEventListener('blur', releaseTalk);
  window.addEventListener('pagehide', () => {
    if (opening || token) disconnect();
  });
  document.addEventListener('visibilitychange', () => {
    if (document.hidden && token) disconnect();
  });
  new MutationObserver(() => {
    show(message);
    showOutput(outputMessage, selectedOutput);
  }).observe(document.documentElement, {
    attributes: true, attributeFilter: ['lang']
  });
  fetch(prefix, {signal: AbortSignal.timeout(2000)}).then(reply => reply.json()).then(info => {
    if (!info.enabled) {
      show('语音对讲未启用');
    } else if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
      const address = new URL(location.href);
      address.protocol = 'https:';
      address.hostname = info.httpsHost;
      address.port = info.httpsPort;
      secureLink.href = address;
      secureLink.hidden = false;
      show('请通过 HTTPS 打开遥控页以使用手机麦克风');
    } else {
      streamEnabled = info.stream === true;
      form.hidden = false;
      show('使用连接设置的账号登录对讲');
    }
  }).catch(() => show('语音对讲未启用'));
})();
