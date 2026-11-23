//! Input equipment and stopped-buffer persistence. The ARM owns the looper.
use super::*;
pub const DIGITAL_CAPACITY: u32 = 62_500 * 60;
const BUFFER: u32 = 0xc0000000;
#[derive(Debug, Clone)]
pub struct DigitalSnapshot {
    pub active: bool,
    pub state: u32,
    pub frames: u32,
    pub position: u32,
    pub revision: u32,
    pub wraps: u32,
    pub source_position: f64,
    pub source_duration: f64,
    pub microphone: bool,
    pub repeat: bool,
    pub eof: bool,
}
impl DigitalSnapshot {
    pub fn capturing(&self) -> bool {
        self.state == 1 || self.state == 3
    }
    pub fn name(&self) -> &'static str {
        match self.state {
            1 => "Recording",
            2 => "Playing",
            3 => "Overdubbing",
            _ if self.frames == 0 => "Empty",
            _ => "Stopped",
        }
    }
}
pub(crate) struct Input {
    pub samples: Vec<f32>,
    hz: u32,
    position: f64,
    pub microphone: bool,
    pub repeat: bool,
    ended: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            hz: 48000,
            position: 0.,
            microphone: false,
            repeat: false,
            ended: false,
        }
    }
}
impl Input {
    pub fn sample(&mut self, mic: f32, capture: bool) -> (f32, bool) {
        if !capture {
            return (0., false);
        }
        if self.microphone {
            return (mic, false);
        }
        if self.samples.is_empty() {
            return (0., false);
        }
        if self.position >= self.samples.len() as f64 {
            if self.repeat {
                self.position %= self.samples.len() as f64;
            } else {
                let finish = !self.ended;
                self.ended = true;
                return (0., finish);
            }
        }
        let at = self.position as usize;
        let a = self.samples[at];
        let b = self.samples.get(at + 1).copied().unwrap_or(a);
        let sample = a + (b - a) * self.position.fract() as f32;
        self.position += f64::from(self.hz) / f64::from(RATE);
        (sample, false)
    }
    pub fn rewind(&mut self) {
        self.position = 0.;
        self.ended = false;
    }
}
impl Engine {
    pub fn set_digital_mode(&mut self, enabled: bool) -> Result<()> {
        self.machine
            .drive_pin(2, 13, enabled)
            .map_err(|e| e.to_string())
    }
    pub fn digital_snapshot(&mut self) -> Result<DigitalSnapshot> {
        let mut v = [0; 7];
        for (i, n) in v.iter_mut().enumerate() {
            *n = self
                .machine
                .read32(0x20000100 + i as u32 * 4)
                .map_err(|e| e.to_string())?;
        }
        let source = &self.digital_input;
        Ok(DigitalSnapshot {
            active: v[0] != 0,
            state: v[1],
            frames: v[2],
            position: v[3],
            revision: v[4],
            wraps: v[5],
            source_position: source.position.min(source.samples.len() as f64)
                / f64::from(source.hz),
            source_duration: source.samples.len() as f64 / f64::from(source.hz),
            microphone: source.microphone,
            repeat: source.repeat,
            eof: !source.samples.is_empty()
                && source.position >= source.samples.len() as f64
                && !source.repeat,
        })
    }
    pub fn load_digital_source(&mut self, samples: Vec<f32>, hz: u32) -> Result<()> {
        if !(8000..=192000).contains(&hz)
            || samples.len() + self.tapes.iter().map(|t| t.source.len()).sum::<usize>()
                > MAX_SOURCE_FRAMES
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("invalid audio or shared input-memory budget exceeded".into());
        }
        self.digital_input = Input {
            samples,
            hz,
            ..Input::default()
        };
        Ok(())
    }
    pub fn set_digital_microphone(&mut self, enabled: bool) {
        self.digital_input.microphone = enabled;
    }
    pub fn set_digital_repeat(&mut self, enabled: bool) {
        self.digital_input.repeat = enabled;
    }
    pub fn rewind_digital_source(&mut self) {
        self.digital_input.rewind();
    }
    pub fn clear_digital(&mut self) -> Result<()> {
        if self.digital_snapshot()?.state != 0 {
            return Err("Stop the digital loop before clearing it".into());
        }
        self.configure_sequence(6, &[0])
    }
    /// Copy a bounded number of stable sample words between audio packets.
    /// A capture or revision change discards the partial snapshot.
    pub fn prepare_digital_snapshot(&mut self, budget: usize) -> Result<bool> {
        let s = self.digital_snapshot()?;
        if s.capturing() {
            self.digital_cache.clear();
            self.digital_cache_key = None;
            return Ok(false);
        }
        if s.frames > DIGITAL_CAPACITY {
            return Err("invalid digital buffer length".into());
        }
        let key = (s.revision, s.frames);
        if self.digital_cache_key != Some(key) {
            self.digital_cache.clear();
            self.digital_cache_key = Some(key);
            self.digital_cache
                .try_reserve(s.frames as usize)
                .map_err(|_| "not enough memory for digital export")?;
        }
        let end = (self.digital_cache.len() + budget).min(s.frames as usize);
        for i in self.digital_cache.len()..end {
            let v = self
                .machine
                .read32(BUFFER + i as u32 * 4)
                .map_err(|e| e.to_string())?;
            self.digital_cache
                .push(((v as i32) << 8 >> 8) as f32 / 8_388_608.);
        }
        Ok(self.digital_cache.len() == s.frames as usize)
    }
    pub fn digital_samples(&mut self) -> Result<Vec<f32>> {
        if !self.prepare_digital_snapshot(DIGITAL_CAPACITY as usize)? {
            return Err("Finish recording or overdubbing first".into());
        }
        Ok(self.digital_cache.clone())
    }
    /// Storage preload while stopped, followed by an ARM-validated UART commit.
    /// No control state or playhead is written by the host.
    pub fn restore_digital(&mut self, samples: Vec<f32>) -> Result<()> {
        if self.frame == 0 {
            self.render_tracks(&[0.; 256], 62500)?;
        }
        if samples.len() > DIGITAL_CAPACITY as usize
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
            || self.digital_snapshot()?.state != 0
        {
            return Err("invalid saved digital loop or loop is running".into());
        }
        let n = samples.len() as u32;
        for (i, sample) in samples.into_iter().enumerate() {
            let word = (sample.clamp(-1., 1. - 1. / 8_388_608.) * 8_388_608.) as i32 as u32;
            self.machine
                .write32(BUFFER + i as u32 * 4, word)
                .map_err(|e| e.to_string())?;
        }
        let revision = self.digital_snapshot()?.revision;
        self.configure_sequence(
            7,
            &[
                (n & 127) as u8,
                ((n >> 7) & 127) as u8,
                ((n >> 14) & 127) as u8,
                ((n >> 21) & 127) as u8,
            ],
        )?;
        for _ in 0..8 {
            self.render_tracks(&[0.; 256], 62500)?;
            let s = self.digital_snapshot()?;
            if s.revision != revision && s.frames == n {
                return Ok(());
            }
        }
        Err("digital storage restore was not acknowledged by ARM".into())
    }
    pub fn export_digital_wav(&mut self) -> Result<Vec<u8>> {
        let samples = self.digital_samples()?;
        if samples.is_empty() {
            return Err("Record a digital loop first".into());
        }
        let mut cursor = Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: RATE,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| e.to_string())?;
        for v in samples {
            writer
                .write_sample((v * 32767.) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
        Ok(cursor.into_inner())
    }
}
