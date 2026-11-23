# Virtual AnsaLoop

Panasonic EASA-PHONE KX-T1421 tape prototype, running the actual Daisy ARM
firmware in Virtual Daisy. This is a separate project from Virtual Synthphone.

1. Power on. Load an audio file, or use the microphone with headphones.
2. Press **REC**. Input starts when the firmware brings the tape up to speed.
3. Press **STOP**, then **PLAY** to loop the recording. A file sends Stop when
   it ends; **Repeat input** keeps it running. Recording replaces tape audio.
4. Stop and wait for the transport to settle. Enter any positive length in
   seconds, including fractions, and press **Insert**. Choose **Saved tapes**
   to return to an earlier cassette. Six and eight seconds are presets.

**R** records, **S** stops, **Space** plays. Drag the whole front selector plate;
its index line snaps under the three printed marks. Its function is unassigned.
The side switches, spare LED and second tape well also remain unassigned.
Speaker volume changes the monitor output; MIDI input is planned.

Audio is decoded locally, mixed to mono and resampled. Nothing is uploaded.
Tapes and the inserted length save in this browser after stopping and on Power
off. Export important recordings as WAV. Closing while recording can lose
unsaved changes. Storage errors are shown; damaged records are retained rather
than overwritten. There is one cassette per length, rounded to a 62,500 Hz
sample. The library has a 256 MiB sample budget; source audio has a separate
128 MiB decoded mono budget and a 100 MB encoded-file limit. These protect
browser memory; tape durations are not restricted to presets or whole seconds.

## Run

Install Rust targets `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`, plus
`wasm-bindgen-cli` **0.2.127**. Reuse a compatible `CARGO_TARGET_DIR`.

```sh
cargo test --release --workspace --locked
node --test assets/tests/*.mjs
cargo run --release -p xtask -- build
cargo run --release -p xtask -- serve
```

Open `http://127.0.0.1:3030`. Inspect preview ownership first and stop the server
after verification. `dist/` works on HTTPS static hosting without a backend.
The tracked `.openai/hosting.json` identifies this project's private Site.

## Firmware and model

The embedded ELF is the unchanged image used by AnsaLoop's ARM + ngspice tests:
`3a2fe358b5a4359c5715a21813e96aa5a0f7e7e7a1745e7a9a96e2dd19d3f2e5`.
The engine verifies its hash before boot. Button presses are GPIO contacts;
actual ARM instructions handle controls, interlocks and serial audio I/O.
Virtual Daisy's opt-in functional runtime preserves instruction and peripheral
behavior while omitting detailed CPU bus/cache timing. Its default timed runtime
is unchanged. No host controller makes firmware decisions for this interface.

Live audio uses AnsaLoop's fast ideal linear tape carrier: 20 ms motor response,
62,500 Hz codec, one-frame ADC latch and four-frame firmware FIFO. Resampling
retains phase across audio packets. Bounded worklet queues report underruns,
dropped capture and recovery. This browser does not run ngspice or model magnetic
tape quality. The separate [AnsaLoop circuit lab](https://github.com/UnitedCode/ansaloop)
contains coupled analog/ARM tests. Panasonic electrical parameters and the donor
machine remain unmeasured. The firmware is a functional tape probe, not the
finished board support package or sequencer.

Rust implements the engine, UI, storage, routing and build tools. JavaScript is
limited to browser module startup and the required AudioWorkletProcessor PCM
bridge. HTML/CSS place controls over a photo-derived straightened view; the
moving selector plate uses pixels from that same image.

[Artwork sources and generation prompts](docs/artwork.md) ·
[Physical controls and current assignments](docs/hardware-inventory.md)

Rebuild the original firmware without source patches:

```sh
cargo run --release -p xtask -- firmware /path/to/ansaloop
```

Review the firmware ABI before deliberately changing the expected hash.
