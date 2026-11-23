# Sequencer and connections

The first sequence is sixteen sixteenth notes, two monophonic tracks sharing
tempo and length. Right is MIDI channel 1, left channel 2. Each step has a MIDI
note and on/off flag. Default 120 BPM gives a two-second bar. Enabled steps send
Note On at the boundary and Note Off halfway through. PLAY restarts at step 1;
STOP releases notes. CUE moves the edit cursor independently of the playhead.

MIDI notes are the primary purpose: connect a synth or drum machine to MIDI OUT.
The optional virtual MIDI synth is test equipment, fed only by actual ARM UART
output. It is not an onboard AnsaLoop synth proposal. The same pattern can open
and close the two tape playback gates. In MIDI mode tapes play uninterrupted;
in Tape mode there are gates but no generated notes; Both does both. Gates are
restored open when stopped. Tapes must already be playing to hear tape rhythms.

The visible physical module remains LED-only. CUE / REC / PLAY / STOP and the
three-position selector provide a simple physical edit/play workflow; the
sixteen bench buttons and numeric fields are virtual programming conveniences.
Pitch entry on real hardware would use MIDI IN. Sequence changes are currently
volatile; tape recordings retain the independent browser libraries.

MIDI IN accepts note entry while stopped and optional Start/Continue/Stop/Clock.
Selecting external clock stops the sequence; it waits for incoming clock to
advance. [MIDI's message definitions](https://midi.org/about-midi-part-3midi-messages)
specify 24 clock pulses per quarter note, giving six per sixteenth note.

Hardware access uses [Web MIDI](https://www.w3.org/TR/webmidi/) only after the
user clicks Connect MIDI devices and selects ports. Default ports are None;
no SysEx permission is requested. Editor SysEx stays in the virtual UART and is
not forwarded to hardware. Output messages are reassembled into complete MIDI
messages before forwarding. A selected output receives note-off control on
power off or port change. Browser/main-thread/audio buffering introduces jitter;
this is a functional demo, not a validated low-latency MIDI interface. Hardware
ports and microphone permissions are not exercised by automated browser QA.

## Proposed audio / MIDI panel

MIDI IN → isolator/receiver → Daisy UART → sequencer → output driver → MIDI OUT.
Audio IN → record amplifier / bias circuit → selected record head.
Playback heads → playback amplifiers / EQ → input selection → Daisy codec →
post-playback analog gates → mixer / output buffer → Audio OUT.

Start with four 3.5 mm panel sockets: MIDI IN and OUT using TRS Type A, and mono
audio IN and OUT. Reserve 16 mm socket-center spacing, an 80 × 18 mm strip and
at least 22 mm behind it. The [Same Sky SJ1-3533NG component drawing](https://www.sameskydevices.com/product/resource/sj1-353xng.pdf)
provides an example 18 × 8 × 12.3 mm body. These are component envelopes and
design reserves, not a verified Panasonic mounting location. Check jack nuts,
plug grip space, cables, rear wall thickness and internal mechanism clearance.
The source photo's telephone sockets are reference markers, not sufficient
evidence that all four replacements fit there.

The [Daisy Seed datasheet](https://daisy.nyc3.cdn.digitaloceanspaces.com/products/seed/Daisy_Seed_datasheet.pdf)
provides two audio inputs and two outputs. The prototype dedicates one codec
channel per deck and switches its input between recording source and head return.
This needs physical analog routing, separate head amplifiers, record drivers and
post-playback gates. Simultaneous tape return plus additional independent stereo
line monitoring/digital loops would need a revised channel budget or extra codec.
Microphone use requires its own suitable preamplifier; the proposed AUDIO IN is
not a claim of a completed mic input circuit. MIDI carries control, not audio.

Both virtual decks operate independently. Whether the donor mechanisms can do
so with their original motor/control arrangement remains unverified. The browser
tape model is linear and synthetic; its new playback-gate response is a smooth
1 ms approximation, not an ngspice-validated gate circuit or measured tape sound.

The later digital-looper mode uses the same right-track gate after the digital
output. The left track still sends MIDI; it has no second digital sample slot.
Selecting Tape/Digital stops the old looper mode, retains its recordings and
returns the front keys to looper control. It does not stop the MIDI sequence.
