# Virtual AnsaLoop

- Keep this interface independent of Virtual Synthphone and its firmware.
- New implementation and build tools are Rust. Browser module bootstrap and
  AudioWorkletProcessor are the only JavaScript platform bridges; they must not
  implement recording, looping, sequencing or instrument control.
- Execute the verified ARM ELF through the pinned Virtual Daisy runtime. Button
  actions are GPIO contacts. Never replace firmware behavior with a host state
  machine or patch firmware to make an interface test pass.
- Reuse the AnsaLoop carrier as a pinned dependency. Distinguish this fast ideal
  linear carrier from the native ngspice co-simulation and physical validation.
- Preserve both cassette contents during swapping, power cycling and errors.
- Input and cassette audio stay in the browser. Never upload recordings.
- Reuse compatible Cargo caches, honor the account usage reserve, and maintain
  500 GB free space. Stop task-owned preview servers when verification ends.
- Verify, commit, push and merge into main under the user's standing authority.
- Reuse .openai/hosting.json identity. Keep the Site private unless asked otherwise.
