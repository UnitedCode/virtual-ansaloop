//! Interactive physical carrier. Instrument decisions execute in the ARM ELF.
use ansaloop_sim::tape::{Attachment, Cassette, Deck, Fault, FRAME_NS, RATE};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, io::Cursor, rc::Rc};
use virtual_daisy::{board::Board, machine::Machine};

pub const FIRMWARE: &[u8] = include_bytes!("../../firmware/ansaloop.elf");
pub const EXPECTED_SHA: &str = "3a2fe358b5a4359c5715a21813e96aa5a0f7e7e7a1745e7a9a96e2dd19d3f2e5";
pub type Result<T> = std::result::Result<T, String>;
pub use ansaloop_sim::control::Command as Button;
// Bound browser allocations, not musical durations. All positive, fractional
// lengths that fit this aggregate 256 MiB media budget are accepted.
pub const MAX_MEDIA_FRAMES: usize = 64 * 1024 * 1024;
pub const MAX_SOURCE_FRAMES: usize = 32 * 1024 * 1024;

pub struct Snapshot {
    pub state: &'static str,
    pub seconds: f64,
    pub tape_frames: u32,
    pub position: f64,
    pub speed: f64,
    pub gpio: u16,
    pub frames: u64,
    pub written: u64,
    pub can_swap: bool,
    pub source_position: f64,
    pub source_duration: f64,
    pub eof: bool,
}
pub struct Engine {
    machine: Machine,
    deck: Rc<RefCell<Deck>>,
    parked: BTreeMap<usize, Cassette>,
    selected: usize,
    frame: u64,
    last_release: u64,
    adc: f32,
    output: f32,
    previous_output: f32,
    phase: f64,
    last_mic: f32,
    source: Vec<f32>,
    source_hz: u32,
    source_position: f64,
    source_ended: bool,
    pub microphone: bool,
    pub repeat_source: bool,
    pub dirty: bool,
}
impl Engine {
    pub fn new() -> Result<Self> {
        let hash = format!("{:x}", Sha256::digest(FIRMWARE));
        if hash != EXPECTED_SHA {
            return Err("firmware hash differs from verified ARM tape image".into());
        }
        let deck = Rc::new(RefCell::new(Deck::new(6, Fault::None)));
        let mut machine =
            Machine::from_elf_functional(Board::Rev5, FIRMWARE).map_err(|e| e.to_string())?;
        machine.attach(Box::new(Attachment(deck.clone())));
        Ok(Self {
            machine,
            deck,
            parked: BTreeMap::from([((8 * RATE) as usize, Cassette::blank(8))]),
            selected: (6 * RATE) as usize,
            frame: 0,
            last_release: 0,
            adc: 0.,
            output: 0.,
            previous_output: 0.,
            phase: 0.,
            last_mic: 0.,
            source: vec![],
            source_hz: 48000,
            source_position: 0.,
            source_ended: false,
            microphone: false,
            repeat_source: false,
            dirty: false,
        })
    }
    /// Queue real contact edges. No host controller tick or desired-state write.
    pub fn press(&mut self, button: Button) {
        let now = self.frame * FRAME_NS;
        let start = (now + FRAME_NS).max(self.last_release + 4_000_000);
        self.last_release = start + 20_000_000;
        let mut deck = self.deck.borrow_mut();
        let keep = deck
            .schedule
            .iter()
            .rposition(|(t, _)| *t <= now)
            .unwrap_or(0);
        deck.schedule.drain(..keep);
        deck.schedule.push((start, Some(button)));
        deck.schedule.push((self.last_release, None));
    }
    pub fn load_source(&mut self, samples: Vec<f32>, hz: u32) -> Result<()> {
        if !(8000..=192000).contains(&hz)
            || samples.len() > MAX_SOURCE_FRAMES
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err(
                "audio must be finite, 8–192 kHz, and fit the browser's input-memory budget".into(),
            );
        }
        self.source = samples;
        self.source_hz = hz;
        self.source_position = 0.;
        self.source_ended = false;
        self.microphone = false;
        Ok(())
    }
    pub fn rewind_source(&mut self) {
        self.source_position = 0.;
        self.source_ended = false;
    }
    fn file_sample(&mut self) -> f32 {
        if self.source.is_empty() {
            return 0.;
        }
        if self.source_position >= self.source.len() as f64 {
            if self.repeat_source {
                self.source_position %= self.source.len() as f64;
            } else {
                if !self.source_ended {
                    self.source_ended = true;
                    self.press(Button::Stop);
                }
                return 0.;
            }
        }
        let index = self.source_position as usize;
        let a = self.source[index];
        let b = self.source.get(index + 1).copied().unwrap_or(a);
        let value = a + (b - a) * self.source_position.fract() as f32;
        self.source_position += f64::from(self.source_hz) / f64::from(RATE);
        value
    }
    fn tick(&mut self, mic: f32) -> Result<f32> {
        let gates = self.deck.borrow().outputs;
        let input = if gates == 15 {
            if self.microphone {
                mic
            } else {
                self.file_sample()
            }
        } else {
            self.deck.borrow().read()
        };
        // Declared one-frame ADC latch, matching the coupled coordinator.
        if self.machine.queued_input_frames() == 0 {
            self.machine
                .input(&[[self.adc, self.adc]])
                .map_err(|e| e.to_string())?;
        }
        self.adc = input;
        self.frame += 1;
        self.machine
            .run_until(self.frame * FRAME_NS, 20_000)
            .map_err(|e| e.to_string())?;
        let audio = self.machine.take_audio();
        if audio.len() > 1 {
            return Err("codec produced multiple frames per exchange".into());
        }
        for sample in audio {
            self.output = sample[0];
            let mut deck = self.deck.borrow_mut();
            let before = deck.written;
            deck.advance(sample[0]);
            self.dirty |= deck.written != before;
        }
        if self.machine.read32(0x20000004).map_err(|e| e.to_string())? != 0 {
            return Err("ARM firmware exception".into());
        }
        Ok(self.output)
    }
    /// Browser packet boundaries never change the engine/audio resampling phase.
    pub fn render(&mut self, mic: &[f32], device_hz: u32) -> Result<Vec<f32>> {
        if !(8000..=192000).contains(&device_hz)
            || mic.len() > 4096
            || mic.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("invalid audio packet".into());
        }
        let ratio = f64::from(RATE) / f64::from(device_hz);
        let mut out = Vec::with_capacity(mic.len());
        for &sample in mic {
            let old_phase = self.phase;
            self.phase += ratio;
            let count = self.phase.floor() as usize;
            for n in 1..=count {
                let fraction = ((n as f64 - old_phase) / ratio) as f32;
                let input = self.last_mic + (sample - self.last_mic) * fraction;
                self.previous_output = self.output;
                self.tick(input.clamp(-1., 1.))?;
            }
            self.phase -= count as f64;
            out.push(
                self.previous_output + (self.output - self.previous_output) * self.phase as f32,
            );
            self.last_mic = sample;
        }
        Ok(out)
    }
    pub fn snapshot(&self) -> Snapshot {
        let d = self.deck.borrow();
        Snapshot {
            state: match d.outputs {
                15 => "Recording",
                3 if d.ready() => "Playing",
                3 => "Starting",
                _ if d.speed > 0.001 => "Stopping",
                _ => "Stopped",
            },
            seconds: self.selected as f64 / f64::from(RATE),
            tape_frames: self.selected as u32,
            position: d.media.as_ref().unwrap().position / f64::from(RATE),
            speed: d.speed,
            gpio: d.outputs,
            frames: self.frame,
            written: d.written,
            can_swap: d.outputs == 0
                && d.speed < 0.001
                && self.frame * FRAME_NS >= self.last_release,
            source_position: self.source_position.min(self.source.len() as f64)
                / f64::from(self.source_hz),
            source_duration: self.source.len() as f64 / f64::from(self.source_hz),
            eof: !self.source.is_empty()
                && self.source_position >= self.source.len() as f64
                && !self.repeat_source,
        }
    }
    pub fn swap(&mut self, seconds: u32) -> Result<()> {
        self.insert_tape(f64::from(seconds))
    }
    fn frames(seconds: f64) -> Result<usize> {
        let count = (seconds * f64::from(RATE)).round();
        if !seconds.is_finite() || seconds <= 0. || count < 1. {
            return Err("enter a positive tape length in seconds".into());
        }
        if count > MAX_MEDIA_FRAMES as f64 {
            return Err("tape exceeds the browser's media-memory budget".into());
        }
        Ok(count as usize)
    }
    fn check_budget(&self, frames: usize) -> Result<()> {
        let known = self.selected == frames || self.parked.contains_key(&frames);
        let total =
            self.selected + self.parked.keys().sum::<usize>() + if known { 0 } else { frames };
        if total > MAX_MEDIA_FRAMES {
            Err("the tape library exceeds the browser's 256 MiB media-memory budget".into())
        } else {
            Ok(())
        }
    }
    pub fn insert_tape(&mut self, seconds: f64) -> Result<()> {
        if !self.snapshot().can_swap {
            return Err("press Stop and wait for the tape to settle before swapping".into());
        }
        let frames = Self::frames(seconds)?;
        if frames == self.selected {
            return Ok(());
        }
        self.check_budget(frames)?;
        let media = if let Some(media) = self.parked.remove(&frames) {
            media
        } else {
            let mut samples = Vec::new();
            samples
                .try_reserve_exact(frames)
                .map_err(|_| "not enough browser memory for this tape")?;
            samples.resize(frames, 0.);
            Cassette {
                samples,
                position: 0.,
                wraps: 0,
            }
        };
        match self.deck.borrow_mut().replace(media) {
            Ok(old) => {
                self.parked.insert(self.selected, old);
                self.selected = frames;
                self.dirty = true;
                Ok(())
            }
            Err((reason, media)) => {
                self.parked.insert(frames, media);
                Err(reason.into())
            }
        }
    }
    pub fn tape(&self, seconds: u32) -> Result<Vec<f32>> {
        self.tape_frames(Self::frames(f64::from(seconds))? as u32)
    }
    pub fn tape_frames(&self, frames: u32) -> Result<Vec<f32>> {
        let frames = frames as usize;
        Ok(if self.selected == frames {
            self.deck.borrow().media.as_ref().unwrap().samples.clone()
        } else {
            self.parked
                .get(&frames)
                .ok_or("unknown cassette")?
                .samples
                .clone()
        })
    }
    pub fn catalog(&self) -> Vec<f64> {
        let mut frames = self
            .parked
            .keys()
            .copied()
            .chain(std::iter::once(self.selected))
            .collect::<Vec<_>>();
        frames.sort_unstable();
        frames
            .into_iter()
            .map(|f| f as f64 / f64::from(RATE))
            .collect()
    }
    pub fn tapes(&self) -> Vec<(u32, Vec<f32>)> {
        self.parked
            .iter()
            .map(|(&frames, t)| (frames as u32, t.samples.clone()))
            .chain(std::iter::once((
                self.selected as u32,
                self.deck.borrow().media.as_ref().unwrap().samples.clone(),
            )))
            .collect()
    }
    pub fn restore(&mut self, seconds: u32, samples: Vec<f32>) -> Result<()> {
        let frames = Self::frames(f64::from(seconds))? as u32;
        self.restore_frames(frames, samples)
    }
    pub fn restore_frames(&mut self, frames: u32, samples: Vec<f32>) -> Result<()> {
        let frames = frames as usize;
        if !self.snapshot().can_swap
            || frames == 0
            || frames > MAX_MEDIA_FRAMES
            || samples.len() != frames
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("invalid saved cassette".into());
        }
        self.check_budget(frames)?;
        if frames == self.selected {
            self.deck.borrow_mut().media.as_mut().unwrap().samples = samples;
        } else {
            self.parked.insert(
                frames,
                Cassette {
                    samples,
                    position: 0.,
                    wraps: 0,
                },
            );
        }
        Ok(())
    }
    pub fn export_wav(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(44 + self.selected * 2)
            .map_err(|_| "not enough browser memory to export this tape")?;
        let mut cursor = Cursor::new(bytes);
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
        for value in &self.deck.borrow().media.as_ref().unwrap().samples {
            writer
                .write_sample((*value * 32767.) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
        Ok(cursor.into_inner())
    }
    pub fn audio_status(&mut self) -> Result<(u32, u32)> {
        Ok((
            self.machine.read32(0x40015818).map_err(|e| e.to_string())?,
            self.machine.read32(0x40015838).map_err(|e| e.to_string())?,
        ))
    }
}
