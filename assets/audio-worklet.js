// AudioWorkletProcessor is a JavaScript-only browser API. PCM transport only.
// No instrument control, looping, recording or effects run here.
class AnsaLoopAudio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.blocks = []; this.offset = 0; this.queued = 0; this.playing = false;
    this.underruns = 0; this.recoveries = 0; this.inFlight=0; this.captureDrops=0;
    this.port.onmessage = ({data}) => {
      if (data.connect) {
        this.bridge = data.connect;
        this.bridge.onmessage = ({data}) => {
          if (data.audio) {
            this.inFlight=Math.max(0,this.inFlight-1);
            this.blocks.push(data.audio); this.queued += data.audio.length;
            if (this.queued > sampleRate * 0.12) {
              while (this.blocks.length > 2) {this.queued -= this.blocks.shift().length-this.offset;this.offset=0;}
              this.recoveries++;
            }
          }
          if (data.reset) { this.blocks = []; this.offset = 0; this.queued = 0; this.playing = false; }
        };
        this.bridge.start();
      }
    };
  }
  process(inputs, outputs) {
    const out = outputs[0][0];
    if (!this.playing && this.queued >= sampleRate * 0.016) this.playing = true;
    for (let i=0; i<out.length; i++) {
      if (this.playing && this.blocks.length) {
        out[i] = this.blocks[0][this.offset++]; this.queued--;
        if (this.offset === this.blocks[0].length) {this.blocks.shift(); this.offset=0;}
      } else {
        out[i]=0;
        if (this.playing) {this.underruns++; this.playing=false;}
      }
    }
    for (let c=1; c<outputs[0].length; c++) outputs[0][c].set(out);
    if (this.bridge && this.inFlight < 8) {
      const mic = new Float32Array(out.length);
      const channels=inputs[0]??[];
      for (const channel of channels) for (let i=0;i<mic.length;i++) mic[i]+=channel[i]/channels.length;
      this.inFlight++;
      this.bridge.postMessage({mic, hz:sampleRate, underruns:this.underruns, recoveries:this.recoveries, captureDrops:this.captureDrops, queued:this.queued},[mic.buffer]);
    } else if (this.bridge) {
      this.captureDrops++;
    }
    return true;
  }
}
registerProcessor('ansaloop-audio', AnsaLoopAudio);
