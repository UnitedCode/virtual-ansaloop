# Verification — 2026-11-22

## Automated checks

- Seven native release integration tests passed with the pinned functional ARM
  runtime: actual six/eight-second record/play/swap; blank-tape bypass rejection
  and source EOF Stop contact; exact audio across different packet boundaries;
  saved-media corruption rejection; microphone PCM through the ARM codec;
  1.25-second record/play loop correlation above 0.999; 120.5-second cassette
  allocation, full library restoration and 7,531,250-frame WAV export.
- Three Node tests passed for the required AudioWorkletProcessor PCM bridge:
  bounded input backpressure, exact PCM/prebuffer/underrun, overflow recovery
  with a partially consumed block.
- Rust format and strict Clippy passed for the native workspace and WASM UI.
- Rebuilding the untouched AnsaLoop ARM source reproduced embedded SHA256
  `3a2fe358b5a4359c5715a21813e96aa5a0f7e7e7a1745e7a9a96e2dd19d3f2e5`.

Dependencies: AnsaLoop `069a2e72e807df66141a5c685009d4fd56985a1d`;
Virtual Daisy `035c9d99291781496c4864e03386cee9648fa01c`.
The SDK's full native/CPU regression suites and strict all-target Clippy passed
before its functional-runtime change was merged. AnsaLoop's native tests,
including coupled analog/ARM regression, passed against that SDK revision.

## Browser observations

Checked in the in-app browser on localhost:3030, in a separate test origin from
the user's annotated preview. The user's original tab and recordings were
preserved.

- Photo-derived plate moves as a whole; keyboard steps and pointer dragging
  move its index line among the three printed marks. A drag from first to third
  position reports position 3, with no assigned firmware action.
- Inserted 1.25- and 120.5-second tapes and selected the saved eight-second tape.
- Loaded a local synthetic 6.2-second WAV and recorded onto the 1.25-second loop.
  Firmware GPIO reached 15 while recording, then zero at source EOF; 387,520
  carrier frames were written. Playback reached GPIO 3 with nonzero output RMS
  (0.1924 in one observed packet). Swapping was disabled while moving.
- Native Export WAV download produced mono Int16 PCM at 62,500 Hz, exactly
  78,125 samples / 1.25 seconds. After Power off, page reload and Power on,
  the active custom length and recording returned. Export SHA256 before and
  after was identical:
  `42e9c8ef56e9277645c31a4df9780e0de8bf501562f1351fadfc6640c8ffd406`.
- An earlier six-second browser recording and eight-second native recording
  also passed; the functional runtime advanced 39.893 virtual seconds in
  approximately 39.88 wall seconds in an earlier live browser check.

These establish simulated behavior and file integrity, not subjective audio
quality or measured Panasonic behavior. Actual microphone permission/capture,
physical tape mechanics and analog calibration remain unverified. Live audio
uses the fast linear carrier; ngspice remains in the separate native circuit lab.
