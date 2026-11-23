//! Two physical carriers controlled by one actual dual-deck ARM firmware image.
pub use ansaloop_sim::control::Command as Button;
use ansaloop_sim::tape::{Cassette, Deck, Fault, FRAME_NS, RATE};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, io::Cursor, rc::Rc};
use virtual_daisy::{
    board::Board,
    machine::{Carrier, Machine},
};
pub const FIRMWARE: &[u8] = include_bytes!("../../firmware/ansaloop.elf");
pub const EXPECTED_SHA: &str = "517fd66b84c1a86edcd59e2d33c2f351ff35bf4beaa085ca9e69b0fdaf1a63a8";
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_MEDIA_FRAMES: usize = 64 * 1024 * 1024;
pub const MAX_SOURCE_FRAMES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckId {
    Right,
    Left,
}
impl DeckId {
    pub const ALL: [Self; 2] = [Self::Right, Self::Left];
    pub const fn index(self) -> usize {
        match self {
            Self::Right => 0,
            Self::Left => 1,
        }
    }
    pub fn from_index(index: usize) -> Result<Self> {
        Self::ALL
            .get(index)
            .copied()
            .ok_or_else(|| "unknown deck".into())
    }
}
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
    pub microphone: bool,
    pub repeat: bool,
}
struct Tape {
    deck: Rc<RefCell<Deck>>,
    parked: BTreeMap<usize, Cassette>,
    selected: usize,
    last_release: u64,
    source: Vec<f32>,
    source_hz: u32,
    source_position: f64,
    source_ended: bool,
    microphone: bool,
    repeat: bool,
}
impl Tape {
    fn new(seconds: u32) -> Self {
        let other = if seconds == 6 { 8 } else { 6 };
        Self {
            deck: Rc::new(RefCell::new(Deck::new(seconds, Fault::None))),
            parked: BTreeMap::from([((other * RATE) as usize, Cassette::blank(other))]),
            selected: (seconds * RATE) as usize,
            last_release: 0,
            source: vec![],
            source_hz: 48000,
            source_position: 0.,
            source_ended: false,
            microphone: false,
            repeat: false,
        }
    }
    fn media_frames(&self) -> usize {
        self.selected + self.parked.keys().sum::<usize>()
    }
    fn known(&self, frames: usize) -> bool {
        self.selected == frames || self.parked.contains_key(&frames)
    }
    fn press(&mut self, button: Button, frame: u64) {
        let now = frame * FRAME_NS;
        let start = (now + FRAME_NS).max(self.last_release + 4_000_000);
        self.last_release = start + 20_000_000;
        let mut d = self.deck.borrow_mut();
        let keep = d.schedule.iter().rposition(|(t, _)| *t <= now).unwrap_or(0);
        d.schedule.drain(..keep);
        d.schedule.push((start, Some(button)));
        d.schedule.push((self.last_release, None));
    }
    fn input(&mut self, mic: f32, frame: u64) -> f32 {
        if self.deck.borrow().outputs != 15 {
            return self.deck.borrow().read();
        }
        if self.microphone {
            return mic;
        }
        if self.source.is_empty() {
            return 0.;
        }
        if self.source_position >= self.source.len() as f64 {
            if self.repeat {
                self.source_position %= self.source.len() as f64;
            } else {
                if !self.source_ended {
                    self.source_ended = true;
                    self.press(Button::Stop, frame);
                }
                return 0.;
            }
        }
        let at = self.source_position as usize;
        let a = self.source[at];
        let b = self.source.get(at + 1).copied().unwrap_or(a);
        let sample = a + (b - a) * self.source_position.fract() as f32;
        self.source_position += f64::from(self.source_hz) / f64::from(RATE);
        sample
    }
    fn snapshot(&self, frame: u64) -> Snapshot {
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
            frames: frame,
            written: d.written,
            can_swap: d.outputs == 0 && d.speed < 0.001 && frame * FRAME_NS >= self.last_release,
            source_position: self.source_position.min(self.source.len() as f64)
                / f64::from(self.source_hz),
            source_duration: self.source.len() as f64 / f64::from(self.source_hz),
            eof: !self.source.is_empty()
                && self.source_position >= self.source.len() as f64
                && !self.repeat,
            microphone: self.microphone,
            repeat: self.repeat,
        }
    }
}
mod sequencer;
pub use sequencer::SequenceState;
struct DualAttachment([Rc<RefCell<Deck>>; 2]);
impl Carrier for DualAttachment {
    fn pins(&mut self, outputs: &[u16; 11], now: u64) -> [(u16, u16); 11] {
        let mut pins = [(0, 0); 11];
        for (i, cell) in self.0.iter().enumerate() {
            let mut d = cell.borrow_mut();
            let gates = (outputs[0] >> (4 * i)) & 15;
            if d.outputs != gates {
                d.trace.push((now, gates));
                d.outputs = gates;
            }
            d.now = now;
            let command = d
                .schedule
                .iter()
                .rev()
                .find(|(at, _)| *at <= now)
                .and_then(|(_, c)| *c);
            let (media, ready, play, record, stop) = if i == 0 {
                (8, 9, 10, 11, 12)
            } else {
                (6, 7, 4, 5, 13)
            };
            pins[1].0 |= 1 << stop;
            pins[1].1 |= u16::from(command == Some(Button::Stop)) << stop;
            pins[2].0 |= (1 << media) | (1 << ready) | (1 << play) | (1 << record);
            pins[2].1 |= (u16::from(d.media.is_some()) << media)
                | (u16::from(d.ready()) << ready)
                | (u16::from(command == Some(Button::Play)) << play)
                | (u16::from(command == Some(Button::Record)) << record);
        }
        pins
    }
    fn next_event(&self) -> Option<u64> {
        self.0
            .iter()
            .filter_map(|cell| {
                let d = cell.borrow();
                d.schedule
                    .iter()
                    .find(|(at, _)| *at > d.now)
                    .map(|(at, _)| *at)
            })
            .min()
    }
}
pub struct Engine {
    machine: Machine,
    tapes: [Tape; 2],
    frame: u64,
    adc: [f32; 2],
    output: [f32; 2],
    previous_output: [f32; 2],
    phase: f64,
    last_mic: f32,
    monitor: [f32; 2],
    previous_monitor: [f32; 2],
    gate_gain: [f32; 2],
    cue_release: u64,
    serial: Vec<(u64, u8)>,
    receiver: sequencer::Receiver,
    // Compatibility aliases for the original right-deck API.
    pub microphone: bool,
    pub repeat_source: bool,
    pub dirty: bool,
    pub dirty_decks: [bool; 2],
}
impl Engine {
    pub fn new() -> Result<Self> {
        if format!("{:x}", Sha256::digest(FIRMWARE)) != EXPECTED_SHA {
            return Err("firmware hash differs from verified dual-deck ARM image".into());
        }
        let tapes = [Tape::new(6), Tape::new(8)];
        let mut machine =
            Machine::from_elf_functional(Board::Rev5, FIRMWARE).map_err(|e| e.to_string())?;
        machine.attach(Box::new(DualAttachment([
            tapes[0].deck.clone(),
            tapes[1].deck.clone(),
        ])));
        Ok(Self {
            machine,
            tapes,
            frame: 0,
            adc: [0.; 2],
            output: [0.; 2],
            previous_output: [0.; 2],
            phase: 0.,
            last_mic: 0.,
            monitor: [0.; 2],
            previous_monitor: [0.; 2],
            gate_gain: [1.; 2],
            cue_release: 0,
            serial: Vec::new(),
            receiver: Default::default(),
            microphone: false,
            repeat_source: false,
            dirty: false,
            dirty_decks: [false; 2],
        })
    }
    pub fn press(&mut self, button: Button) {
        self.press_deck(DeckId::Right, button);
    }
    pub fn sequence_mode(&mut self, enabled: bool, track: DeckId) -> Result<()> {
        self.machine
            .drive_pin(1, 14, enabled)
            .map_err(|e| e.to_string())?;
        self.machine
            .drive_pin(1, 15, track == DeckId::Left)
            .map_err(|e| e.to_string())
    }
    pub fn cue_button(&mut self) -> Result<()> {
        self.machine
            .drive_pin(2, 12, true)
            .map_err(|e| e.to_string())?;
        self.cue_release = self.frame + 1250;
        Ok(())
    }
    pub fn midi_input(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 1024 {
            return Err("MIDI packet too long".into());
        }
        self.machine
            .receive_serial(bytes)
            .map_err(|e| e.to_string())
    }
    /// Experimental non-commercial SysEx editor protocol, received by actual ARM UART.
    pub fn configure_sequence(&mut self, command: u8, args: &[u8]) -> Result<()> {
        if command > 127 || args.iter().any(|b| *b > 127) {
            return Err("invalid MIDI data".into());
        }
        let mut bytes = vec![0xf0, 0x7d, command];
        bytes.extend_from_slice(args);
        bytes.push(0xf7);
        self.midi_input(&bytes)
    }
    pub fn sequence_state(&mut self) -> Result<SequenceState> {
        let mut v = [0; 10];
        for (i, n) in v.iter_mut().enumerate() {
            *n = self
                .machine
                .read32(0x20000020 + i as u32 * 4)
                .map_err(|e| e.to_string())?;
        }
        let mut notes = [[0; 16]; 2];
        for (t, track) in notes.iter_mut().enumerate() {
            for (s, n) in track.iter_mut().enumerate() {
                *n = self
                    .machine
                    .read32(0x20000060 + (t * 16 + s) as u32 * 4)
                    .map_err(|e| e.to_string())? as u8;
            }
        }
        Ok(SequenceState {
            running: v[0] != 0,
            step: v[1] as usize,
            cursor: v[2] as usize,
            bpm: v[3],
            masks: [v[4] as u16, v[5] as u16],
            target: v[6],
            external: v[7] != 0,
            track: v[8] as usize,
            overflow: v[9],
            notes,
        })
    }
    pub fn take_midi(&mut self) -> Vec<(u64, u8)> {
        std::mem::take(&mut self.serial)
    }
    pub fn test_instrument(&mut self, enabled: bool) {
        self.receiver.enabled = enabled;
    }
    pub fn press_deck(&mut self, id: DeckId, button: Button) {
        self.tapes[id.index()].press(button, self.frame);
    }
    pub fn set_microphone(&mut self, id: DeckId, enabled: bool) {
        self.tapes[id.index()].microphone = enabled;
        if id == DeckId::Right {
            self.microphone = enabled;
        }
    }
    pub fn set_repeat(&mut self, id: DeckId, enabled: bool) {
        self.tapes[id.index()].repeat = enabled;
        if id == DeckId::Right {
            self.repeat_source = enabled;
        }
    }
    pub fn load_source(&mut self, samples: Vec<f32>, hz: u32) -> Result<()> {
        self.load_source_deck(DeckId::Right, samples, hz)
    }
    pub fn load_source_deck(&mut self, id: DeckId, samples: Vec<f32>, hz: u32) -> Result<()> {
        let other = 1 - id.index();
        if !(8000..=192000).contains(&hz)
            || samples.len() + self.tapes[other].source.len() > MAX_SOURCE_FRAMES
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err(
                "audio must be finite, 8–192 kHz, and fit the shared input-memory budget".into(),
            );
        }
        let t = &mut self.tapes[id.index()];
        t.source = samples;
        t.source_hz = hz;
        t.source_position = 0.;
        t.source_ended = false;
        self.set_microphone(id, false);
        Ok(())
    }
    pub fn rewind_source(&mut self) {
        self.rewind_source_deck(DeckId::Right);
    }
    pub fn rewind_source_deck(&mut self, id: DeckId) {
        let t = &mut self.tapes[id.index()];
        t.source_position = 0.;
        t.source_ended = false;
    }
    fn tick(&mut self, mic: f32) -> Result<[f32; 2]> {
        let input = [
            self.tapes[0].input(mic, self.frame),
            self.tapes[1].input(mic, self.frame),
        ];
        if self.machine.queued_input_frames() == 0 {
            self.machine.input(&[self.adc]).map_err(|e| e.to_string())?;
        }
        self.adc = input;
        self.frame += 1;
        if self.cue_release != 0 && self.frame >= self.cue_release {
            self.machine
                .drive_pin(2, 12, false)
                .map_err(|e| e.to_string())?;
            self.cue_release = 0;
        }
        self.machine
            .run_until(self.frame * FRAME_NS, 20_000)
            .map_err(|e| e.to_string())?;
        let audio = self.machine.take_audio();
        if audio.len() > 1 {
            return Err("codec produced multiple frames per exchange".into());
        }
        for sample in audio {
            self.output = sample;
            for (id, value) in sample.into_iter().enumerate() {
                let mut d = self.tapes[id].deck.borrow_mut();
                let before = d.written;
                d.advance(value);
                self.dirty_decks[id] |= d.written != before;
                self.dirty |= d.written != before;
            }
        }
        for event in self.machine.take_serial() {
            self.receiver.byte(event.1);
            if self.serial.len() < 8192 {
                self.serial.push(event);
            }
        }
        let gpio = self.machine.read32(0x58020014).map_err(|e| e.to_string())?;
        let synth = self.receiver.sample();
        for i in 0..2 {
            let target = if gpio & (1 << (8 + i)) != 0 { 1. } else { 0. };
            self.gate_gain[i] += (target - self.gate_gain[i]) * 0.016;
            self.monitor[i] = self.output[i] * self.gate_gain[i] + synth;
        }
        if self.machine.read32(0x20000004).map_err(|e| e.to_string())? != 0 {
            return Err("ARM firmware exception".into());
        }
        Ok(self.output)
    }
    /// Two codec channels: right deck on channel 0, left deck on channel 1.
    pub fn render_tracks(&mut self, mic: &[f32], device_hz: u32) -> Result<Vec<[f32; 2]>> {
        self.render_output(mic, device_hz, false)
    }
    /// Proposed post-codec tape gates plus the external MIDI test instrument.
    pub fn render_monitor(&mut self, mic: &[f32], device_hz: u32) -> Result<Vec<[f32; 2]>> {
        self.render_output(mic, device_hz, true)
    }
    fn render_output(
        &mut self,
        mic: &[f32],
        device_hz: u32,
        monitor: bool,
    ) -> Result<Vec<[f32; 2]>> {
        if !(8000..=192000).contains(&device_hz)
            || mic.len() > 4096
            || mic.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("invalid audio packet".into());
        }
        self.tapes[0].microphone = self.microphone;
        self.tapes[0].repeat = self.repeat_source;
        let ratio = f64::from(RATE) / f64::from(device_hz);
        let mut out = Vec::with_capacity(mic.len());
        for &sample in mic {
            let old = self.phase;
            self.phase += ratio;
            let count = self.phase.floor() as usize;
            for n in 1..=count {
                let fraction = ((n as f64 - old) / ratio) as f32;
                let input = self.last_mic + (sample - self.last_mic) * fraction;
                self.previous_output = self.output;
                self.previous_monitor = self.monitor;
                self.tick(input.clamp(-1., 1.))?;
            }
            self.phase -= count as f64;
            out.push(std::array::from_fn(|i| {
                let (before, now) = if monitor {
                    (self.previous_monitor[i], self.monitor[i])
                } else {
                    (self.previous_output[i], self.output[i])
                };
                before + (now - before) * self.phase as f32
            }));
            self.last_mic = sample;
        }
        Ok(out)
    }
    /// Original single-deck API returns the right deck. Both transports advance;
    /// the browser mixes the two channels returned by render_tracks instead.
    pub fn render(&mut self, mic: &[f32], hz: u32) -> Result<Vec<f32>> {
        Ok(self
            .render_tracks(mic, hz)?
            .into_iter()
            .map(|v| v[0])
            .collect())
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot_deck(DeckId::Right)
    }
    pub fn snapshot_deck(&self, id: DeckId) -> Snapshot {
        self.tapes[id.index()].snapshot(self.frame)
    }
    fn frames(seconds: f64) -> Result<usize> {
        let n = (seconds * f64::from(RATE)).round();
        if !seconds.is_finite() || seconds <= 0. || n < 1. {
            return Err("enter a positive tape length in seconds".into());
        }
        if n > MAX_MEDIA_FRAMES as f64 {
            return Err("tape exceeds the browser's media-memory budget".into());
        }
        Ok(n as usize)
    }
    fn check_budget(&self, id: DeckId, frames: usize) -> Result<()> {
        let added = if self.tapes[id.index()].known(frames) {
            0
        } else {
            frames
        };
        if self.tapes.iter().map(Tape::media_frames).sum::<usize>() + added > MAX_MEDIA_FRAMES {
            Err("both tape libraries exceed the shared 256 MiB media-memory budget".into())
        } else {
            Ok(())
        }
    }
    pub fn swap(&mut self, seconds: u32) -> Result<()> {
        self.insert_tape(f64::from(seconds))
    }
    pub fn insert_tape(&mut self, seconds: f64) -> Result<()> {
        self.insert_tape_deck(DeckId::Right, seconds)
    }
    pub fn insert_tape_deck(&mut self, id: DeckId, seconds: f64) -> Result<()> {
        if !self.snapshot_deck(id).can_swap {
            return Err("press Stop and wait for this deck to settle before swapping".into());
        }
        let frames = Self::frames(seconds)?;
        if frames == self.tapes[id.index()].selected {
            return Ok(());
        }
        self.check_budget(id, frames)?;
        let t = &mut self.tapes[id.index()];
        let media = if let Some(media) = t.parked.remove(&frames) {
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
        let result = t.deck.borrow_mut().replace(media);
        match result {
            Ok(old) => {
                t.parked.insert(t.selected, old);
                t.selected = frames;
                self.dirty = true;
                self.dirty_decks[id.index()] = true;
                Ok(())
            }
            Err((reason, media)) => {
                t.parked.insert(frames, media);
                Err(reason.into())
            }
        }
    }
    pub fn tape(&self, seconds: u32) -> Result<Vec<f32>> {
        self.tape_frames(Self::frames(f64::from(seconds))? as u32)
    }
    pub fn tape_frames(&self, frames: u32) -> Result<Vec<f32>> {
        self.tape_frames_deck(DeckId::Right, frames)
    }
    pub fn tape_frames_deck(&self, id: DeckId, frames: u32) -> Result<Vec<f32>> {
        let t = &self.tapes[id.index()];
        let n = frames as usize;
        Ok(if n == t.selected {
            t.deck.borrow().media.as_ref().unwrap().samples.clone()
        } else {
            t.parked.get(&n).ok_or("unknown cassette")?.samples.clone()
        })
    }
    pub fn catalog(&self) -> Vec<f64> {
        self.catalog_deck(DeckId::Right)
    }
    pub fn catalog_deck(&self, id: DeckId) -> Vec<f64> {
        let t = &self.tapes[id.index()];
        let mut n = t
            .parked
            .keys()
            .copied()
            .chain(std::iter::once(t.selected))
            .collect::<Vec<_>>();
        n.sort_unstable();
        n.into_iter().map(|f| f as f64 / f64::from(RATE)).collect()
    }
    pub fn tapes(&self) -> Vec<(u32, Vec<f32>)> {
        self.tapes_deck(DeckId::Right)
    }
    pub fn tapes_deck(&self, id: DeckId) -> Vec<(u32, Vec<f32>)> {
        let t = &self.tapes[id.index()];
        t.parked
            .iter()
            .map(|(&n, c)| (n as u32, c.samples.clone()))
            .chain(std::iter::once((
                t.selected as u32,
                t.deck.borrow().media.as_ref().unwrap().samples.clone(),
            )))
            .collect()
    }
    pub fn restore(&mut self, seconds: u32, samples: Vec<f32>) -> Result<()> {
        self.restore_frames(Self::frames(f64::from(seconds))? as u32, samples)
    }
    pub fn restore_frames(&mut self, frames: u32, samples: Vec<f32>) -> Result<()> {
        self.restore_frames_deck(DeckId::Right, frames, samples)
    }
    pub fn restore_frames_deck(
        &mut self,
        id: DeckId,
        frames: u32,
        samples: Vec<f32>,
    ) -> Result<()> {
        let n = frames as usize;
        if !self.snapshot_deck(id).can_swap
            || n == 0
            || n > MAX_MEDIA_FRAMES
            || samples.len() != n
            || samples.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("invalid saved cassette".into());
        }
        self.check_budget(id, n)?;
        let t = &mut self.tapes[id.index()];
        if n == t.selected {
            t.deck.borrow_mut().media.as_mut().unwrap().samples = samples;
        } else {
            t.parked.insert(
                n,
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
        self.export_wav_deck(DeckId::Right)
    }
    pub fn export_wav_deck(&self, id: DeckId) -> Result<Vec<u8>> {
        let t = &self.tapes[id.index()];
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(44 + t.selected * 2)
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
        for value in &t.deck.borrow().media.as_ref().unwrap().samples {
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
