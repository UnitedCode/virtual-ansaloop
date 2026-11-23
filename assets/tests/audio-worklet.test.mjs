// The AudioWorklet JavaScript-only boundary is tested in its own runtime.
import test from 'node:test';
import assert from 'node:assert/strict';
globalThis.sampleRate=48000;
globalThis.AudioWorkletProcessor=class{constructor(){this.port={};}};
let Processor;
globalThis.registerProcessor=(_,klass)=>Processor=klass;
await import('../audio-worklet.js');
function bridge(p){const packets=[];const port={postMessage:data=>packets.push(data),start(){}};p.port.onmessage({data:{connect:port}});return {port,packets};}
function process(p,input=new Float32Array(128)){const out=[new Float32Array(128),new Float32Array(128)];p.process([[input]],[out]);return out;}
test('capture backpressure bounds pending packets and reports dropped input',()=>{
 const p=new Processor();const {port,packets}=bridge(p);
 for(let i=0;i<100;i++)process(p);
 assert.equal(packets.length,8);assert.equal(p.inFlight,8);assert.equal(p.captureDrops,92);
 port.onmessage({data:{audio:new Float32Array(128)}});process(p);
 assert.equal(packets.length,9);assert.equal(packets.at(-1).captureDrops,92);
});
test('PCM crosses unchanged, playback prebuffers, and an underrun is reported',()=>{
 const p=new Processor();const {port}=bridge(p);const audio=new Float32Array(1024).fill(.25);
 port.onmessage({data:{audio}});const out=process(p);
 assert.ok(out[0].every(x=>x===.25));assert.deepEqual(out[0],out[1]);
 for(let i=0;i<10;i++)process(p);
 assert.equal(p.underruns,1);assert.equal(p.queued,0);
});
test('overflow recovery keeps queue accounting correct with a partial block',()=>{
 const p=new Processor();const {port}=bridge(p);
 port.onmessage({data:{audio:new Float32Array(1024).fill(.1)}});process(p);
 for(let i=0;i<50;i++)port.onmessage({data:{audio:new Float32Array(128).fill(.2)}});
 const remaining=p.blocks.reduce((n,b)=>n+b.length,0)-p.offset;
 assert.equal(p.queued,remaining);assert.ok(p.queued<=sampleRate*.12);assert.ok(p.recoveries>0);
});
