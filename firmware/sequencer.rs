//! Fixed-memory, sample-clocked MIDI sequencer. No host clock or tape seeks.
pub struct Sequencer {
    pub digital_request: Option<(u8, u32)>,
    pub running: bool,
    pub step: u8,
    pub cursor: u8,
    pub track: usize,
    pub bpm: u16,
    pub target: u8, // 0 MIDI, 1 tape gates, 2 both
    pub external: bool,
    pub notes: [[u8; 16]; 2],
    pub masks: [u16; 2],
    pub gates: u8,
    phase: u32,
    clocks: u8,
    sounding: [u8; 2],
    tx: [u8; 128],
    head: usize,
    tail: usize,
    pub overflow: u32,
    sysex: bool,
    packet: [u8; 8],
    len: usize,
    status: u8,
    first: Option<u8>,
}
impl Sequencer {
    pub const fn new() -> Self {
        Self {
            digital_request: None,
            running: false,
            step: 0,
            cursor: 0,
            track: 0,
            bpm: 120,
            target: 0,
            external: false,
            notes: [
                [
                    60, 62, 64, 67, 64, 62, 60, 55, 60, 62, 64, 67, 69, 67, 64, 62,
                ],
                [48; 16],
            ],
            masks: [0x1111, 0x4444],
            gates: 3,
            phase: 0,
            clocks: 0,
            sounding: [255; 2],
            tx: [0; 128],
            head: 0,
            tail: 0,
            overflow: 0,
            sysex: false,
            packet: [0; 8],
            len: 0,
            status: 0,
            first: None,
        }
    }
    fn emit(&mut self, b: u8) {
        let next = (self.head + 1) % self.tx.len();
        if next == self.tail {
            self.overflow += 1;
        } else {
            self.tx[self.head] = b;
            self.head = next;
        }
    }
    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            return None;
        }
        let b = self.tx[self.tail];
        self.tail = (self.tail + 1) % self.tx.len();
        Some(b)
    }
    fn note(&mut self, track: usize, on: bool, note: u8) {
        for b in [
            if on { 0x90 } else { 0x80 } | track as u8,
            note,
            if on { 100 } else { 0 },
        ] {
            self.emit(b);
        }
    }
    fn release(&mut self) {
        for t in 0..2 {
            if self.sounding[t] != 255 {
                self.note(t, false, self.sounding[t]);
                self.sounding[t] = 255;
            }
        }
    }
    fn trigger(&mut self) {
        self.release();
        let mut gates = 0;
        for t in 0..2 {
            if self.masks[t] & (1 << self.step) != 0 {
                gates |= 1 << t;
                if self.target != 1 {
                    let n = self.notes[t][self.step as usize];
                    self.note(t, true, n);
                    self.sounding[t] = n;
                }
            }
        }
        self.gates = if self.target == 0 { 3 } else { gates };
    }
    pub fn start(&mut self, transmit: bool) {
        self.release();
        self.running = true;
        self.step = 0;
        self.phase = 0;
        self.clocks = 0;
        if transmit {
            self.emit(0xfa);
        }
        self.trigger();
    }
    pub fn stop(&mut self, transmit: bool) {
        self.running = false;
        self.release();
        self.gates = 3;
        if transmit {
            self.emit(0xfc);
        }
    }
    pub fn cue(&mut self) {
        self.cursor = (self.cursor + 1) % 16;
    }
    pub fn toggle(&mut self) {
        self.masks[self.track] ^= 1 << self.cursor;
    }
    fn clock(&mut self) {
        if !self.running {
            return;
        }
        self.clocks += 1;
        if self.clocks == 3 {
            self.release();
            if self.target != 0 {
                self.gates = 0;
            }
        }
        if self.clocks == 6 {
            self.clocks = 0;
            self.step = (self.step + 1) % 16;
            self.trigger();
        }
    }
    pub fn sample(&mut self) {
        if self.running && !self.external {
            self.phase += u32::from(self.bpm) * 24;
            if self.phase >= 62_500 * 60 {
                self.phase -= 62_500 * 60;
                self.emit(0xf8);
                self.clock();
            }
        }
    }
    fn configure(&mut self) {
        if self.len < 2 || self.packet[0] != 0x7d {
            return;
        }
        let p = self.packet;
        match (p[1], self.len) {
            (6, 3) if p[2] == 0 => self.digital_request = Some((6, 0)),
            (7, 6) => {
                self.digital_request = Some((
                    7,
                    u32::from(p[2])
                        | (u32::from(p[3]) << 7)
                        | (u32::from(p[4]) << 14)
                        | (u32::from(p[5]) << 21),
                ))
            }
            (1, 4) => self.bpm = (u16::from(p[2]) | (u16::from(p[3]) << 7)).clamp(30, 240),
            (2, 6) if p[2] < 2 && p[3] < 16 => {
                let t = p[2] as usize;
                let s = p[3] as usize;
                self.notes[t][s] = p[4];
                if p[5] != 0 {
                    self.masks[t] |= 1 << s;
                } else {
                    self.masks[t] &= !(1 << s);
                }
                self.cursor = p[3];
            }
            (3, 3) if p[2] < 3 => {
                self.release();
                self.target = p[2];
                self.gates = 3;
            }
            (4, 3) => {
                self.stop(true);
                self.external = p[2] != 0;
            }
            (5, 3) if p[2] < 16 => self.cursor = p[2],
            _ => (),
        }
    }
    pub fn receive(&mut self, b: u8) {
        // Realtime messages may interrupt any MIDI message, including SysEx.
        if b >= 0xf8 {
            if self.external {
                match b {
                    0xf8 => self.clock(),
                    0xfa => self.start(false),
                    0xfb => self.running = true,
                    0xfc => self.stop(false),
                    _ => (),
                }
            }
            return;
        }
        if b == 0xf0 {
            self.sysex = true;
            self.len = 0;
            self.status = 0;
            self.first = None;
            return;
        }
        if b == 0xf7 {
            if self.sysex {
                self.configure();
            }
            self.sysex = false;
            return;
        }
        if b & 0x80 != 0 {
            self.sysex = false;
            self.status = b;
            self.first = None;
            return;
        }
        if self.sysex {
            if self.len < self.packet.len() {
                self.packet[self.len] = b;
                self.len += 1;
            } else {
                self.sysex = false;
            }
        } else if matches!(self.status & 0xf0, 0x80..=0xb0 | 0xe0) {
            if let Some(note) = self.first.take() {
                if self.status & 0xf0 == 0x90 && b != 0 && !self.running {
                    self.notes[self.track][self.cursor as usize] = note;
                    self.masks[self.track] |= 1 << self.cursor;
                    self.cue();
                }
            } else {
                self.first = Some(b);
            }
        }
    }
}
