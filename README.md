# Virtual AnsaLoop

Panasonic EASA-PHONE KX-T1421 prototype: two tape loops, a digital looper and a 16-step MIDI
sequencer, running real Daisy ARM firmware in Virtual Daisy. Separate from
Virtual Synthphone. [Play online](https://unitedcode.github.io/virtual-ansaloop/).

## Try the sequencer

1. **Power on**, enable **SEQ controls**, then press the phone's **PLAY** key.
   The default two-track pattern plays through **Hear virtual MIDI synth**.
2. **CUE** advances the edit cursor. **REC** toggles that step; **STOP** stops
   the sequence. LEFT / RIGHT selects the track, on MIDI channels 2 / 1.
3. Select a step in the bench editor to change its note or enabled state.
   Set BPM or choose **Loop gates** / **MIDI + loop gates** for rhythmic muting.
4. To hear tape gates, disable SEQ controls, **Play both** recorded tapes, then
   enable SEQ and press PLAY. Tapes keep moving; the gates only change the mix.
5. Expand **MIDI input / output** for optional hardware ports. Nothing is
   connected automatically. A keyboard enters notes while stopped; incoming
   clock can drive the sequence. **Enter note** tests the same UART input path.

The whole front selector plate moves and snaps under the printed marks.
Proposed assignment: **left tape / SEQ / right tape**. The bench's 16 buttons
are editing tools, not added physical buttons. The lid module contains only
LEDs. Sequence patterns reset to the demo on power cycling; tape libraries save.

## Record tape loops

Power on and disable SEQ controls. Choose LEFT / RIGHT, load audio or enable
the microphone with headphones. REC records; STOP then PLAY loops the tape.
Recording replaces existing audio. Repeat input keeps a short source running.
**R / S / Space** mirror the front keys; **Right arrow** mirrors CUE.

Stop the selected deck and wait for it to settle, then enter any positive tape
length, including fractions, and press Insert. Saved tapes retain one cassette
per sample-rounded length per deck. Both decks can play or record independently.
Speaker volume affects monitoring, not recorded media or WAV exports.

Audio stays local. Libraries save after stopping and on Power off. Export
important recordings; closing during recording can lose unsaved changes. The
original library remains on the right; left uses separate IndexedDB keys.
Tapes share 256 MiB; decoded mono sources share 128 MiB, with a 100 MB file limit.

## Run and verify

Install Rust targets `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`, plus
`wasm-bindgen-cli` **0.2.127**. Reuse a compatible `CARGO_TARGET_DIR`.

```sh
cargo test --release --workspace --locked
node --test assets/tests/*.mjs
cargo run --release -p xtask -- build
cargo run --release -p xtask -- serve
```

Open `http://127.0.0.1:3030`. Check preview ownership first; stop owned servers
after verification. The public website uses GitHub Pages from prebuilt files on
`gh-pages`, with `.nojekyll`. Build locally and publish only a verified source
revision. GitHub performs hosting deployment; it does not compile Rust. The old
private Sites project is retained but is not the publication target.

## What is simulated

One verified ARM ELF controls both transports, MIDI parsing, note generation,
24-PPQN clock, CUE editing and post-codec tape-gate GPIO. Virtual Daisy executes
the ARM instructions and peripherals in its functional mode; detailed bus/cache
timing is omitted. There is no host sequencer or JavaScript instrument logic.
The browser's virtual MIDI synth is a separate Rust test receiver listening to
actual baud-paced USART output. Optional Web MIDI forwarding has browser/queue
jitter; external equipment and live MIDI sync have not been physically tested.

The browser uses the pinned AnsaLoop fast ideal linear tape carrier: synthetic
20 ms motors, 62,500 Hz codec, one-frame ADC latch and four-frame firmware FIFO.
GPIO gates model proposed post-playback analog switches with a 1 ms smoothing
response; they do not alter recording signals or seek tapes. The separate
[circuit lab](https://github.com/UnitedCode/ansaloop) runs ngspice with the earlier
single-deck image; this sequencer image has not been coupled to ngspice.

The raised LED module is **150 × 24 × 15 mm mounted**, including a
144 × 18 × 1.6 mm PCB and 3 mm LEDs on 8 mm centers. Photo scale is estimated
from a cassette. Exact case fit, hinge clearance, mechanism independence, head
circuits and donor electrical parameters remain unmeasured. This is not the
finished physical board support package. Expand the PCB drawing under the photo.

- [Firmware, GPIO and UART protocol](firmware/README.md)
- [Sequencer behavior and proposed connections](docs/sequencer.md)
- [LED component drawings and fit assumptions](docs/sequencer-fit.md)
- [Physical controls](docs/hardware-inventory.md) · [Artwork](docs/artwork.md)

`cargo run --release -p xtask -- firmware` rebuilds the ARM image; review its ABI
and deliberately update the expected SHA256. `xtask layout` regenerates the
dimensioned SVGs. Rust owns the engine, UI, storage and tools; JavaScript is
limited to module startup and the mandatory AudioWorkletProcessor PCM bridge.

## Digital looper

Choose **Digital loop** (or flip the two-position side switch), load audio or
use the microphone, then press **REC**. **PLAY** closes the recording and loops
that length. **REC** during playback toggles overdub; **STOP** keeps the loop;
**CUE** restarts its playhead. REC while stopped starts a replacement recording.
A file closes and starts looping automatically at EOF unless Repeat is enabled.

The single digital slot is mono, up to 60 seconds at the prototype's 62,500 Hz
codec rate. Its sample buffer and all record/play/overdub logic live in ARM
firmware, separate from the two simulated cassettes. Overdubbing adds input to
stored PCM with signed 24-bit saturation. Clear works only while stopped.
Switching Tape/Digital stops the old audio mode and retains all three recordings;
press PLAY in the new mode to resume it. The sequencer can continue independently.
Loop gates use the right track for the digital loop, or each track's tape deck.

The browser saves the digital buffer separately from both cassette libraries,
after capture/overdub ends and on Power off. Export WAV works during playback or
while stopped. There is no JavaScript looper or host playback buffer replacing
ARM execution. Browser persistence preloads only stopped sample data into the
reserved SDRAM window, then sends a UART command for ARM to validate the length.
Physical nonvolatile storage, SDRAM setup/timing and the complete board support
package still need implementation/verification on hardware. The functional
runtime validates the looper's instructions and memory behavior, not SDRAM timing.

The runtime pin includes [Virtual Daisy PR #20](https://github.com/UnitedCode/virtual-daisy/pull/20),
which implements SSAT rather than changing the firmware's compiled saturation.

Building from source currently requires access to the pinned AnsaLoop simulator
repository. The GitHub Pages build is precompiled and needs no repository access.
