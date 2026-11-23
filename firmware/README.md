# Firmware provenance

`ansaloop.elf` is the unmodified Rust tape probe built from
[UnitedCode/ansaloop](https://github.com/UnitedCode/ansaloop) firmware/tape.rs,
firmware/tape.ld and src/control.rs at `ee54bf9881cac1cf8313c7d958e9570c42c37eb7`.
It is byte-identical to the ARM image used in the passing six/eight-second
coupled circuit tests. SHA256:

`3a2fe358b5a4359c5715a21813e96aa5a0f7e7e7a1745e7a9a96e2dd19d3f2e5`

`cargo run --release -p xtask -- firmware /path/to/ansaloop` reproduces it with
the normal thumbv7em-none-eabihf rustc build flags. It is a functional probe;
do not flash it to a physical mechanism. No firmware processing runs in JS.
