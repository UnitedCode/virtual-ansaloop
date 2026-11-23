# Sequencer verification — 2026-11-22

ARM ELF SHA256: `517fd66b84c1a86edcd59e2d33c2f351ff35bf4beaa085ca9e69b0fdaf1a63a8`.
The engine checks this hash before booting Virtual Daisy.

- `cargo test --release --workspace --locked`: 14 passed. Existing tape tests
  retain independent recording, simultaneous playback, fractional/long tapes,
  restore/export, EOF contacts, microphone PCM and packet-boundary behavior.
- New actual-ARM tests verify CUE advances, REC toggles a step without writing
  tape, PLAY emits MIDI Start/notes/24-PPQN clock, and STOP releases notes.
  At 120 BPM, successive completed UART clock bytes are 20.75–20.92 ms apart.
- UART SysEx changes BPM and notes; stopped MIDI Note On enters a note and moves
  the cursor. External-clock steps do not advance with host audio time alone;
  six received Timing Clocks advance exactly one step.
- Tape-gate tests observe silence/signal changes while both transport outputs
  remain engaged, head positions advance and original tape data stays identical.
- Strict native and WASM Clippy, Rust formatting, standalone firmware formatting
  and `git diff --check` pass. The three existing Node PCM-bridge tests pass.
- Browser QA on localhost loaded the final ARM hash, advanced the LED playhead,
  showed SEQ PLAYING with nonzero monitor output and no stream recoveries,
  exercised CUE/REC, and entered MIDI note 72 through the virtual UART. Numeric
  BPM/note fields now send input edits immediately. Both existing cassette
  libraries and inserted fractional lengths survived reloads.

This is simulated functional validation. No physical MIDI ports, microphone
permission, donor tape mechanism, LED shift-register electronics or new gate
circuit were tested. There was no listening assessment. The browser uses the
linear tape model; the new image has not run in the native ngspice coordinator.
GitHub Pages deployment and its source revision are recorded in the publishing
commit on `gh-pages`; the live firmware hash is `firmware-sha256.txt`.
