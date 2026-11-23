class IntercomCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.enabled = false;
    this.phase = 0;
    this.sum = 0;
    this.count = 0;
    this.frame = new ArrayBuffer(1920);
    this.view = new DataView(this.frame);
    this.used = 0;
    this.port.onmessage = ({data}) => {
      if (data === 'start') {
        this.enabled = true;
        this.phase = this.sum = this.count = this.used = 0;
      } else {
        this.enabled = false;
        this.send();
        this.port.postMessage('flushed');
      }
    };
  }

  send() {
    if (!this.used) return;
    const pcm = this.frame.slice(0, this.used);
    this.port.postMessage(pcm, [pcm]);
    this.used = 0;
  }

  process(inputs) {
    const samples = inputs[0]?.[0];
    if (!this.enabled || !samples) return true;
    for (const sample of samples) {
      this.sum += sample;
      this.count++;
      this.phase += 16000;
      if (this.phase < sampleRate) continue;
      this.phase -= sampleRate;
      const pcm = Math.max(-1, Math.min(1, this.sum / this.count));
      this.view.setInt16(this.used, Math.round(pcm * 32767), true);
      this.used += 2;
      this.sum = this.count = 0;
      if (this.used === 1920) this.send();
    }
    return true;
  }
}

registerProcessor('intercom-capture', IntercomCapture);
