# KX-T1421 controls available for AnsaLoop

Source: the user's eleven annotated browser comments on the original device
photo, 2026-11-22. These are owner observations, not electrical measurements.

| Comment | Hardware | Prototype / planning status |
| --- | --- | --- |
| 1 | Front three-position selector | Proposed left tape / SEQ / right tape; moving index plate |
| 2 | Power indicator | Shows virtual power / completed firmware boot |
| 3 | Play indicator | Follows actual firmware playback state |
| 4 | Record indicator | Follows actual firmware recording state |
| 5 | Second cassette slot | Independent left virtual loop deck, alongside the existing right deck |
| 6 | Additional indicator | Spare LED, unassigned |
| 7 | Side volume knob | Temporary speaker-monitor volume, after firmware audio |
| 8 | Side three-position switch | Position can be explored; function unassigned |
| 9 | Side two-position switch | Position can be explored; function unassigned |
| 10 | Power input | Virtual power connection; physical voltage/polarity unknown |
| 11 | Two telephone connectors | Reference locations; proposed four-jack MIDI IN/OUT and audio IN/OUT panel needs clearance checks |

The fourth front key under CUE advances the sequencer edit cursor. In SEQ mode
REC toggles a step, PLAY starts the sequence and STOP releases notes. LEFT / RIGHT
selects its track. Outside SEQ, REC / PLAY / STOP operate the selected tape.
MIDI parsing and sequencing run inside ARM via USART1. Optional Web MIDI ports
are a browser bridge; physical MIDI input isolation/output drivers remain to be
designed. The photo microphone marker uses the existing virtual microphone route.
The side switches remain unassigned. LED displays follow firmware state; the
physical TLC59282 LED driver and final GPIO allocation are not implemented yet.

The dual-deck firmware/carrier implements two independent synthetic transports
on one virtual Daisy. This is a proposed replacement design, not a claim that the
stock Panasonic can run both mechanisms independently. Its motor arrangement,
routing, head types, sensors and actuator independence still need characterization.

The proposed lid module is 150 × 24 × 15 mm including its mounting pad. It has
real component and PCB envelopes and a six-wire hinge harness proposal. It is
raised above the lid and assumes no cavity beneath it. Case scale is an estimate;
module attachment, lid travel and wiring clearance remain to be verified.

The speaker monitor gain represents the physical volume control's intended
role. It does not change recorded media or exported tape audio. Whether the
real knob belongs in an analog output circuit or a future Daisy ADC/DSP path
remains a board-design decision.

Photo controls and connection markers are positioned from the source image and
annotations. They are not a fabrication drawing. Firmware transport controls
remain GPIO contacts; no host controller replaces ARM execution.

The two-position side switch now provisionally selects Tape / Digital. In digital
mode both tape transports stop and the same front keys control a mono RAM looper;
CUE restarts it. The other side selector and spare LED remain unassigned. The
bench's Tape loops / Digital loop buttons mirror this side switch, not proposed
extra buttons in the physical case. Switching retains all recordings.
