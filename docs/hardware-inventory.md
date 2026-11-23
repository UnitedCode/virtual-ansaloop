# KX-T1421 controls available for AnsaLoop

Source: the user's eleven annotated browser comments on the original device
photo, 2026-11-22. These are owner observations, not electrical measurements.

| Comment | Hardware | Prototype / planning status |
| --- | --- | --- |
| 1 | Front three-position selector | Position can be explored; function unassigned |
| 2 | Power indicator | Shows virtual power / completed firmware boot |
| 3 | Play indicator | Follows actual firmware playback state |
| 4 | Record indicator | Follows actual firmware recording state |
| 5 | Second cassette slot | Independent left virtual loop deck, alongside the existing right deck |
| 6 | Additional indicator | Spare LED, unassigned |
| 7 | Side volume knob | Temporary speaker-monitor volume, after firmware audio |
| 8 | Side three-position switch | Position can be explored; function unassigned |
| 9 | Side two-position switch | Position can be explored; function unassigned |
| 10 | Power input | Virtual power connection; physical voltage/polarity unknown |
| 11 | Two telephone connectors | Proposed replacements: MIDI input and microphone input |

The microphone marker uses the existing virtual microphone route. MIDI remains
planned: the current tape probe has no MIDI parser/sequencer. Do not label that
port as operational MIDI. Switch movements have no assigned firmware behavior;
the interface states that explicitly. LED displays are inferred from existing
transport outputs, not claims of four implemented physical LED GPIO drivers.

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
