# Virtual AnsaLoop

Panasonic EASA-PHONE KX-T1421 tape prototype, running the actual Daisy ARM
firmware in Virtual Daisy. This is a separate project from Virtual Synthphone.

1. Power on. Click the **LEFT / RIGHT** button or cassette to select a deck.
   Load an audio file for that deck, or use the microphone with headphones.
2. Press **REC**. Input starts when the firmware brings the tape up to speed.
3. Press **STOP**, then **PLAY** to loop the recording. A file sends Stop when
   it ends; **Repeat input** keeps it running. Recording replaces tape audio.
4. Stop and wait for the transport to settle. Enter any positive length in
   seconds, including fractions, and press **Insert**. Choose **Saved tapes**
   to return to an earlier cassette. Six and eight seconds are presets.
5. Select the other deck and repeat. Both decks can play or record at once,
   with separate audio and lengths. **Play both / Stop both** operate both;
   the physical front keys and shortcuts operate the selected deck.

**R** records, **S** stops, **Space** plays. Drag the whole front selector plate;
its index line snaps under the three printed marks. Its function is unassigned.
The side switches and spare LED remain unassigned. Both tape wells are active.
Speaker volume changes the monitor output; MIDI input is planned.

The sequencer preview uses a proposed **150 × 24 × 14 mm raised module** plus
a **1 mm mounting pad**. It contains a 144 × 18 × 1.6 mm PCB, sixteen 3 mm LEDs
on 8 mm centers, driver, connector and internal fasteners. Expand **LED module**
under the photo to show the PCB and dimensioned drawing. Four static LEDs are
lit; there is no sequencer behavior yet. Photo scale is estimated from cassette
size; exact KX-T1421 dimensions, lid structure and hinge clearance are unverified.
[Component drawings and fit assumptions](docs/sequencer-fit.md).

Audio is decoded locally, mixed to mono and resampled. Nothing is uploaded.
Each deck's tapes and inserted length save in this browser after stopping and on Power
off. Export important recordings as WAV. Closing while recording can lose
unsaved changes. Storage errors are shown; damaged records are retained rather
than overwritten. The original single-deck library remains on the right deck;
the left deck uses separate keys. There is one cassette per length **per deck**,
rounded to a 62,500 Hz sample. Both libraries share a 256 MiB sample budget; source audio shares a separate
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
The tracked `.openai/hosting.json` identifies the existing private Site. Current
development is local-only at the user's request; do not publish updates.

## Firmware and model

The current ELF implements two independent transport controllers in one Daisy:
`03f0fcd226c6406124c5cfc43690a3d5ea4b014ff87d52c9178a4b852895c497`.
The engine verifies its hash before boot. Button presses are GPIO contacts;
actual ARM instructions handle controls, interlocks and serial audio I/O.
Virtual Daisy's opt-in functional runtime preserves instruction and peripheral
behavior while omitting detailed CPU bus/cache timing. Its default timed runtime
is unchanged. No host controller makes firmware decisions for this interface.
Channel 0 carries the right deck; channel 1 carries the left. Separate recording
signals are retained; the browser models a proposed equal-gain analog monitor
mix. [Firmware source and provisional GPIO ABI](firmware/README.md) are included.

Live audio uses AnsaLoop's fast ideal linear tape carrier: 20 ms motor response,
62,500 Hz codec, one-frame ADC latch and four-frame firmware FIFO. Resampling
retains phase across audio packets. Bounded worklet queues report underruns,
dropped capture and recovery. This browser does not run ngspice or model magnetic
tape quality. The separate [AnsaLoop circuit lab](https://github.com/UnitedCode/ansaloop)
contains coupled analog/ARM tests of the earlier single-deck probe; the new dual
image has not been validated in that ngspice runner. Panasonic electrical parameters and the donor
machine remain unmeasured. The firmware is a functional tape probe, not the
finished board support package or sequencer.

Rust implements the engine, UI, storage, routing and build tools. JavaScript is
limited to browser module startup and the required AudioWorkletProcessor PCM
bridge. HTML/CSS place controls over a photo-derived straightened view; the
moving selector plate uses pixels from that same image.

[Artwork sources and generation prompts](docs/artwork.md) ·
[Physical controls and current assignments](docs/hardware-inventory.md)

Rebuild this project's dual-deck firmware or regenerate the mechanical study:

```sh
cargo run --release -p xtask -- firmware
cargo run --release -p xtask -- layout
```

Review the firmware ABI before deliberately changing the expected hash.
