//! UART transport and an external MIDI test instrument; sequencing lives in ARM.
#[derive(Debug, Clone)]
pub struct SequenceState {
    pub running: bool,
    pub step: usize,
    pub cursor: usize,
    pub bpm: u32,
    pub masks: [u16; 2],
    pub target: u32,
    pub external: bool,
    pub track: usize,
    pub overflow: u32,
    pub notes: [[u8; 16]; 2],
}
pub(crate) struct Receiver {
    status: u8,
    first: Option<u8>,
    voices: [(u8, f32, f32, f32); 2],
    pub enabled: bool,
}
impl Default for Receiver {
    fn default() -> Self {
        Self {
            status: 0,
            first: None,
            voices: [(0, 0., 0., 0.); 2],
            enabled: true,
        }
    }
}
impl Receiver {
    pub fn byte(&mut self, b: u8) {
        if b >= 0xf8 {
            return;
        }
        if b & 128 != 0 {
            self.status = b;
            self.first = None;
            return;
        }
        if let Some(note) = self.first.take() {
            let channel = (self.status & 15) as usize;
            if channel >= 2 {
                return;
            }
            match self.status & 0xf0 {
                0x90 if b != 0 => {
                    self.voices[channel] = (
                        note,
                        0.,
                        f32::from(b) / 127.,
                        440. * 2f32.powf((f32::from(note) - 69.) / 12.) / 62500.,
                    )
                }
                0x80 | 0x90 if self.voices[channel].0 == note => self.voices[channel].2 = 0.,
                _ => (),
            }
        } else {
            self.first = Some(b);
        }
    }
    pub fn sample(&mut self) -> f32 {
        let mut out = 0.;
        for (_, phase, amp, increment) in &mut self.voices {
            if *amp == 0. {
                continue;
            }
            *phase = (*phase + *increment).fract();
            out += (4. * (*phase - 0.5).abs() - 1.) * *amp * 0.13;
            *amp *= 0.99994;
        }
        if self.enabled {
            out
        } else {
            0.
        }
    }
}
