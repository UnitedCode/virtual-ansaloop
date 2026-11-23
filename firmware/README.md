# Dual-deck firmware

`ansaloop.elf` is built from the tracked `dual-tape.rs`, `control.rs` and `tape.ld`:

```sh
cargo run --release -p xtask -- firmware
```

SHA256: `03f0fcd226c6406124c5cfc43690a3d5ea4b014ff87d52c9178a4b852895c497`.
The engine verifies this exact image before booting one Virtual Daisy.

`control.rs` and `tape.ld` are copied from UnitedCode/ansaloop revision
`069a2e72e807df66141a5c685009d4fd56985a1d`. The dual firmware extends that
project's tape probe with two independent instances of the same controller,
button scanning and per-channel audio gating. This is a device feature change,
not a modified image substituted to bypass a test. The earlier one-deck image
remains in Git history and in the separate native analog-coupling lab.

| Proposed probe interface | Right deck | Left deck |
| --- | --- | --- |
| Motor / engage / record / erase | PA0…PA3 | PA4…PA7 |
| Play / Record contacts | PC10 / PC11 | PC4 / PC5 |
| Stop contact | PB12 | PB13 |
| Media present / speed ready | PC8 / PC9 | PC6 / PC7 |
| SAI/codec channel | 0 | 1 |

These are a **virtual carrier ABI**, not a reviewed Daisy pin allocation or
Panasonic wiring diagram. Both controllers execute inside the same ARM ISR,
using TIM2 timing and four stereo frames per SAI IRQ. The host supplies physical
contact levels, readiness and tape audio; it never calls Controller::tick.
The separate output channels feed each deck's recording path; an equal-gain
monitor mix models a proposed external analog mixer.

The LED module is a static mechanical mockup and has no assigned firmware GPIO.
The stock Panasonic's simultaneous transport capability, real head circuits,
motor drivers and microphone routing remain unverified. This probe is not a
production board support package and must not be flashed to an uncharacterized
physical transport.
