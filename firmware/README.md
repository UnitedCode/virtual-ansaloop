# Dual-deck sequencer firmware

Build the tracked `dual-tape.rs`, `control.rs`, `sequencer.rs` and `tape.ld`:

```sh
cargo run --release -p xtask -- firmware
```

The current SHA256 is pinned by `engine/src/lib.rs::EXPECTED_SHA`; the engine
rejects a different image. `control.rs` and `tape.ld` originate from AnsaLoop
`069a2e72e807df66141a5c685009d4fd56985a1d`. The earlier single-deck probe remains
in Git history and the separate native ngspice lab.

| Virtual carrier signal | Right | Left |
| --- | --- | --- |
| Motor / engage / record / erase | PA0…PA3 | PA4…PA7 |
| Play / Record contacts | PC10 / PC11 | PC4 / PC5 |
| Stop contact | PB12 | PB13 |
| Media / ready inputs | PC8 / PC9 | PC6 / PC7 |
| Raw SAI codec channel | 0 | 1 |
| Post-codec audio gate output | PA8 | PA9 |

CUE is PC12. PB14 selects sequencer controls; PB15 selects the left sequence
track. These are provisional virtual signals, **not a verified Daisy carrier
pin allocation**. The host supplies contact levels; ARM scans them every 1.024 ms.
The transport controllers continue running in SEQ mode. CUE advances the edit
cursor, REC toggles that step, PLAY restarts the sequence and STOP releases notes.
Neither tape speed nor head position is reset by the sequencer.

USART1 uses HSI64 / BRR2048 = 31,250 baud, 8N1. The SAI ISR polls RX/TX, with a
127-byte software TX ring and an exposed overflow counter. Internal clock uses
a sample accumulator at 62,500 Hz: 24 MIDI clocks per quarter, six per step,
50% note/gate duration. MIDI Start/Continue/Stop and incoming Timing Clock are
handled when external clock is selected; no clock messages are echoed back.
Two tracks send full Note On/Off messages on channels 1 and 2. Incoming Note On
while stopped enters a note at the cursor and advances it (running status and
interleaved realtime supported for channel messages).

The virtual editor sends experimental non-commercial SysEx through the UART:
`F0 7D command args… F7`, all args seven-bit.

| Command | Arguments |
| --- | --- |
| 1 | BPM low7, BPM high7; clamped 30…240 |
| 2 | track (0/1), step (0…15), note (0…127), enabled (0/1) |
| 3 | target: 0 MIDI, 1 tape gates, 2 both |
| 4 | external clock (0/1); stops sequence before switching |
| 5 | cursor (0…15) |

Read-only diagnostics at `0x20000020` are ten little-endian words: running,
playhead, cursor, BPM, right mask, left mask, target, external clock, edit track,
TX overflow. At `0x20000060`, 32 words hold right then left notes. The host reads
these to draw LEDs; it never writes sequence RAM. The physical TLC59282 LED
shift-register driver is not implemented yet. The firmware initializes its own
static state explicitly; the standalone linker has no general C startup.

PA8/PA9 drive proposed external playback gates. Raw DAC signals still feed
recording unchanged. Virtual mixing and the external MIDI test synth are host
carrier/test-equipment models, not substitutes for firmware sequencing.
This image has not run on the physical donor or in the native ngspice lab.
Motor drivers, head circuits, MIDI electrical interfaces and the full physical
board support package still require design and measurement.
