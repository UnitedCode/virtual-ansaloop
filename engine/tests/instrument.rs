use std::time::Instant;
use virtual_ansaloop_engine::{Button, Engine};

fn tone(hz: f64, rate: u32, seconds: u32) -> Vec<f32> {
    (0..rate * seconds)
        .map(|i| {
            let t = f64::from(i) / f64::from(rate);
            (0.3 * (std::f64::consts::TAU * (hz * t + 2.731 * t * t)).sin()) as f32
        })
        .collect()
}
fn render(engine: &mut Engine, seconds: f64) -> Vec<f32> {
    let n = (seconds * 48000.).round() as usize;
    let mut output = Vec::with_capacity(n);
    for offset in (0..n).step_by(128) {
        output.extend(
            engine
                .render(&vec![0.; (n - offset).min(128)], 48000)
                .unwrap(),
        );
    }
    output
}
fn energy(samples: &[f32]) -> f64 {
    (samples.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / samples.len().max(1) as f64)
        .sqrt()
}
fn stop(engine: &mut Engine) {
    engine.press(Button::Stop);
    render(engine, 0.3);
    assert_eq!(engine.snapshot().state, "Stopped");
    assert!(engine.snapshot().can_swap);
}

#[test]
fn actual_arm_records_plays_and_swaps_independent_six_and_eight_second_tapes() {
    let start = Instant::now();
    let mut engine = Engine::new().unwrap();
    engine.load_source(tone(221., 44100, 12), 44100).unwrap();
    engine.press(Button::Record);
    assert_eq!(
        engine.snapshot().gpio,
        0,
        "a UI press cannot directly change firmware GPIO"
    );
    render(&mut engine, 6.25);
    assert_eq!(engine.snapshot().gpio, 15);
    assert!(engine.snapshot().written > 375000);
    assert!(
        engine.swap(8).is_err(),
        "a running exchange must preserve the cassette"
    );
    stop(&mut engine);
    let six = engine.tape(6).unwrap();
    assert!(energy(&six) > 0.15);
    engine.press(Button::Play);
    let playback = render(&mut engine, 18.4);
    let lag = 6 * 48000;
    let a = &playback[12000..playback.len() - lag];
    let b = &playback[12000 + lag..];
    let dot = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum::<f64>();
    let correlation = dot
        / (a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>()
            * b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>())
        .sqrt();
    assert!(correlation > 0.999, "loop correlation {correlation}");
    stop(&mut engine);
    assert_eq!(
        engine.tape(6).unwrap(),
        six,
        "playback altered the retained recording"
    );
    engine.swap(8).unwrap();
    assert_eq!(energy(&engine.tape(8).unwrap()), 0.);
    engine.load_source(tone(703., 48000, 12), 48000).unwrap();
    engine.press(Button::Record);
    render(&mut engine, 8.25);
    stop(&mut engine);
    let eight = engine.tape(8).unwrap();
    assert!(energy(&eight) > 0.15);
    assert_eq!(eight.len(), 500000);
    engine.swap(6).unwrap();
    assert_eq!(engine.tape(6).unwrap(), six);
    engine.swap(8).unwrap();
    assert_eq!(engine.tape(8).unwrap(), eight);
    let bytes = engine.export_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(reader.spec().sample_rate, 62500);
    assert_eq!(reader.duration(), 500000);
    let (rx, tx) = engine.audio_status().unwrap();
    assert_eq!((rx & 1, tx & 1), (0, 0));
    eprintln!("ARM record/play/swap: PASS; correlation={correlation}; 34.1 virtual seconds in {:.2} wall seconds",start.elapsed().as_secs_f64());
}
#[test]
fn loaded_audio_cannot_bypass_blank_tape_and_eof_sends_a_real_stop_contact() {
    let mut engine = Engine::new().unwrap();
    engine.load_source(tone(440., 48000, 1), 48000).unwrap();
    engine.press(Button::Play);
    let blank = render(&mut engine, 0.4);
    assert_eq!(engine.snapshot().state, "Playing");
    assert_eq!(energy(&blank), 0.);
    stop(&mut engine);
    engine.press(Button::Record);
    render(&mut engine, 1.5);
    assert_eq!(engine.snapshot().state, "Stopped");
    assert!(engine.snapshot().eof);
    assert!(engine.snapshot().written > 60000);
    assert!(energy(&engine.tape(6).unwrap()) > 0.06);
}
#[test]
fn packet_boundaries_preserve_firmware_audio_and_recording_exactly() {
    let mut a = Engine::new().unwrap();
    let mut b = Engine::new().unwrap();
    for engine in [&mut a, &mut b] {
        engine.load_source(tone(337., 44100, 2), 44100).unwrap();
        engine.press(Button::Record);
    }
    let whole = render(&mut a, 0.5);
    let sizes = [31, 127, 512, 777, 80];
    let mut split = Vec::new();
    let mut block = 0;
    while split.len() < 24000 {
        let size = sizes[block % sizes.len()].min(24000 - split.len());
        split.extend(b.render(&vec![0.; size], 48000).unwrap());
        block += 1;
    }
    assert_eq!(whole, split);
    assert_eq!(a.tape(6).unwrap(), b.tape(6).unwrap());
    assert_eq!(a.snapshot().frames, b.snapshot().frames);
}
#[test]
fn cassette_restore_preserves_contents_and_rejects_corruption() {
    let mut engine = Engine::new().unwrap();
    let six = tone(200., 62500, 6);
    let eight = tone(600., 62500, 8);
    engine.restore(6, six.clone()).unwrap();
    engine.restore(8, eight.clone()).unwrap();
    assert_eq!(engine.tape(6).unwrap(), six);
    engine.swap(8).unwrap();
    assert_eq!(engine.tape(8).unwrap(), eight);
    let mut invalid = eight.clone();
    invalid[0] = f32::NAN;
    assert!(engine.restore(8, invalid).is_err());
    assert_eq!(engine.tape(8).unwrap(), eight);
    engine.press(Button::Play);
    render(&mut engine, 0.2);
    assert!(engine.restore(8, vec![0.; 500000]).is_err());
    assert_eq!(engine.tape(8).unwrap(), eight);
}

#[test]
fn fractional_tape_records_and_loops_through_the_actual_arm_codec() {
    let mut engine = Engine::new().unwrap();
    engine.insert_tape(1.25).unwrap();
    engine.load_source(tone(290., 48000, 4), 48000).unwrap();
    engine.press(Button::Record);
    render(&mut engine, 1.55);
    stop(&mut engine);
    let tape = engine.tape_frames(78125).unwrap();
    assert!(energy(&tape) > 0.15);
    engine.press(Button::Play);
    let audio = render(&mut engine, 4.1);
    stop(&mut engine);
    let lag = 60000;
    let a = &audio[12000..audio.len() - lag];
    let b = &audio[12000 + lag..];
    let dot = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum::<f64>();
    let correlation = dot
        / (a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>()
            * b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>())
        .sqrt();
    assert!(correlation > 0.999);
    assert_eq!(engine.tape_frames(78125).unwrap(), tape);
    engine.swap(6).unwrap();
    engine.insert_tape(1.25).unwrap();
    assert_eq!(engine.tape_frames(78125).unwrap(), tape);
}
#[test]
fn custom_lengths_exceed_one_minute_and_survive_library_restore_and_wav_export() {
    let mut engine = Engine::new().unwrap();
    engine.insert_tape(120.5).unwrap();
    assert_eq!(engine.snapshot().seconds, 120.5);
    assert_eq!(engine.snapshot().tape_frames, 7531250);
    engine.insert_tape(0.75).unwrap();
    assert!(engine.catalog().contains(&120.5));
    assert!(engine.catalog().contains(&0.75));
    let library = engine.tapes();
    let mut restored = Engine::new().unwrap();
    for (frames, tape) in library {
        restored.restore_frames(frames, tape).unwrap();
    }
    restored.insert_tape(120.5).unwrap();
    let wav = restored.export_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
    assert_eq!(reader.duration(), 7531250);
    assert_eq!(restored.catalog(), engine.catalog());
    for invalid in [0., -1., f64::NAN, f64::INFINITY, 1e12] {
        assert!(restored.insert_tape(invalid).is_err());
        assert_eq!(restored.snapshot().seconds, 120.5);
    }
}
#[test]
fn microphone_packets_are_recorded_by_arm_and_playback_uses_retained_media() {
    let mut engine = Engine::new().unwrap();
    engine.microphone = true;
    engine.press(Button::Record);
    let input = tone(510., 48000, 1);
    for packet in input.chunks(128) {
        engine.render(packet, 48000).unwrap();
    }
    stop(&mut engine);
    let tape = engine.tape(6).unwrap();
    assert!(energy(&tape) > 0.07);
    engine.microphone = false;
    engine.press(Button::Play);
    let audio = render(&mut engine, 6.3);
    assert!(energy(&audio) > 0.06);
    assert_eq!(engine.tape(6).unwrap(), tape);
}
