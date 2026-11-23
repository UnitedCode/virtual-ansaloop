# Digital looper verification — 2026-11-23

ARM image: `2da57712f0e2bee3a89fee774a87a00f4a33565cfcfd9a1b78d3bcab81737cd7`.
The browser executes this verified image through Virtual Daisy, not a host looper.

Nineteen Rust tests and three PCM-bridge tests pass. Five new ARM integration
cases cover recording/looping/overdub/clear, file EOF, all three recordings across
mode changes, WAV export and RAM restore, the full 60-second limit, FIFO health,
and sequencer editing/gating without destroying or stopping the digital loop.
Native/WASM strict Clippy, formatting and diff checks pass. Incremental storage
snapshots invalidate on capture/revision changes and read at most 2,048 sample
words per audio block; saving/exporting do not walk the full buffer in one block.

Browser QA at localhost:

- Recorded a generated 6.2-second chirp through the ARM codec, automatically
  closed/played it at EOF, and saved it in the separate digital IndexedDB key.
- Exported a 6.20032-second WAV; after power-off/reload/restore, exported the
  identical WAV. SHA256 of both:
  `a65c6a21d726568e37bd51d3b89c6df1b5c9662fac81e33b9046cfabe41089da`.
- Overdubbed the restored loop through the same codec input. Revision advanced
  and length stayed 6.20032 seconds. Playback and saving showed no stream
  recoveries after incremental snapshot extraction was introduced.
- Switched to Tape and played the retained right 1.5-second and left 2.25-second
  loops together; both reported engaged playback GPIO and zero new tape writes.
  Switched back to the retained digital loop. The two-position side switch and
  bench mode buttons stay linked. No microphone/hardware MIDI permission was used.

The runtime required actual SSAT instruction support. Virtual Daisy PR #20
implements decoding, shifting, signed saturation and sticky Q, with 534 CPU tests
passing after the concurrent decoder API update. Its standard root checks pass;
standalone CPU strict Clippy has existing unrelated pedantic failures. AnsaLoop
PR #5 keeps its native runtime/Attachment out of WASM carrier-only builds, with a
WASM library check and five native transport tests passing.

Physical SDRAM initialization/timing, nonvolatile hardware storage, analog
routing and donor operation remain unverified. This is functional simulation,
not a physical listening or electrical validation; the new image has not been
run in the native ngspice coordinator.
