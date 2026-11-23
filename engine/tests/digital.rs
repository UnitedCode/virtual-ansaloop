use virtual_ansaloop_engine::{Button, DeckId, Engine, DIGITAL_CAPACITY};
fn audio(e: &mut Engine, frames: usize, value: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity(frames);
    for at in (0..frames).step_by(256) {
        out.extend(
            e.render_tracks(&vec![value; (frames - at).min(256)], 62500)
                .unwrap(),
        );
    }
    out
}
fn boot() -> Engine {
    let mut e = Engine::new().unwrap();
    audio(&mut e, 256, 0.);
    e
}
#[test]
fn arm_records_loops_overdubs_and_clears_without_writing_either_tape() {
    let mut e = boot();
    e.set_digital_mode(true).unwrap();
    audio(&mut e, 256, 0.);
    e.set_digital_microphone(true);
    e.press(Button::Record);
    audio(&mut e, 12000, 0.25);
    let s = e.digital_snapshot().unwrap();
    assert!(s.active);
    assert_eq!(s.state, 1);
    assert!(s.position > 11000);
    assert!(e.digital_samples().is_err());
    e.press(Button::Play);
    audio(&mut e, 1800, 0.25);
    let s = e.digital_snapshot().unwrap();
    assert_eq!(s.state, 2);
    let n = s.frames;
    assert!((12000..14000).contains(&n));
    let first = e.digital_samples().unwrap();
    let output = audio(&mut e, n as usize * 2, 0.9);
    assert!(output.iter().any(|v| (v[0] - 0.25).abs() < 0.00001));
    assert_eq!(
        e.digital_samples().unwrap(),
        first,
        "playback cannot record the live input"
    );
    assert!(e.digital_snapshot().unwrap().wraps >= 2);
    e.press(Button::Record);
    audio(&mut e, n as usize + 2000, 0.9);
    assert_eq!(e.digital_snapshot().unwrap().state, 3);
    e.press(Button::Record);
    audio(&mut e, 1800, 0.9);
    let overdub = e.digital_samples().unwrap();
    assert_eq!(overdub.len(), first.len());
    assert!(overdub.iter().all(|v| v.is_finite() && v.abs() <= 1.));
    assert!(overdub.iter().filter(|v| **v > 0.99).count() > 10000);
    e.press(Button::Stop);
    audio(&mut e, 1800, 0.);
    assert!(audio(&mut e, 2000, 0.7)
        .iter()
        .all(|v| v[0] == 0. && v[1] == 0.));
    for d in DeckId::ALL {
        assert_eq!(e.snapshot_deck(d).written, 0);
        assert_eq!(e.snapshot_deck(d).gpio, 0);
    }
    e.clear_digital().unwrap();
    audio(&mut e, 1000, 0.);
    assert_eq!(e.digital_snapshot().unwrap().frames, 0);
    assert!(e.digital_samples().unwrap().is_empty());
}
#[test]
fn switching_finalizes_digital_and_preserves_all_three_recordings_and_export_restore() {
    let mut e = boot();
    for id in DeckId::ALL {
        e.restore_frames_deck(
            id,
            6250,
            vec![if id == DeckId::Right { 0.2 } else { -0.4 }; 6250],
        )
        .unwrap();
        e.insert_tape_deck(id, 0.1).unwrap();
        e.press_deck(id, Button::Play);
    }
    audio(&mut e, 15000, 0.);
    e.set_digital_mode(true).unwrap();
    audio(&mut e, 10000, 0.);
    for id in DeckId::ALL {
        assert_eq!(e.snapshot_deck(id).gpio, 0);
    }
    e.set_digital_microphone(true);
    e.press(Button::Record);
    audio(&mut e, 8000, -0.35);
    e.set_digital_mode(false).unwrap();
    audio(&mut e, 1000, 0.);
    let s = e.digital_snapshot().unwrap();
    assert!(!s.active);
    assert_eq!(s.state, 0);
    assert!(s.frames > 7000);
    let saved = e.digital_samples().unwrap();
    let wav = e.export_digital_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
    assert_eq!(reader.duration(), s.frames);
    assert_eq!(reader.spec().sample_rate, 62500);
    for id in DeckId::ALL {
        assert_eq!(
            e.tape_frames_deck(id, 6250).unwrap(),
            vec![if id == DeckId::Right { 0.2 } else { -0.4 }; 6250]
        );
        e.press_deck(id, Button::Play);
    }
    assert!(audio(&mut e, 12000, 0.)
        .iter()
        .any(|v| v[0] > 0.1 && v[1] < -0.2));
    e.set_digital_mode(true).unwrap();
    audio(&mut e, 1000, 0.);
    assert_eq!(e.digital_samples().unwrap(), saved);
    assert_eq!(e.digital_snapshot().unwrap().state, 0);
    let mut restored = boot();
    restored.restore_digital(saved.clone()).unwrap();
    assert_eq!(restored.digital_samples().unwrap(), saved);
    assert!(restored.restore_digital(vec![f32::NAN]).is_err());
    assert_eq!(restored.digital_samples().unwrap(), saved);
    restored.set_digital_mode(true).unwrap();
    restored.press(Button::Play);
    assert!(audio(&mut restored, 12000, 0.).iter().any(|v| v[0] < -0.3));
}
#[test]
fn sixty_second_capacity_closes_recording_in_arm_and_keeps_codec_fifos_healthy() {
    let mut e = boot();
    e.set_digital_mode(true).unwrap();
    e.set_digital_microphone(true);
    e.press(Button::Record);
    for _ in 0..(DIGITAL_CAPACITY as usize / 2048 + 3) {
        e.render_tracks(&[0.125; 2048], 62500).unwrap();
    }
    let s = e.digital_snapshot().unwrap();
    assert_eq!(s.frames, DIGITAL_CAPACITY);
    assert_eq!(s.state, 2);
    assert!(s.position < s.frames);
    let (rx, tx) = e.audio_status().unwrap();
    assert_eq!((rx & 1, tx & 1), (0, 0));
    assert!(e
        .restore_digital(vec![0.; DIGITAL_CAPACITY as usize + 1])
        .is_err());
}
#[test]
fn a_file_closes_the_digital_loop_at_eof_through_a_real_play_contact() {
    let mut e = boot();
    e.set_digital_mode(true).unwrap();
    e.load_digital_source(vec![0.3; 8000], 62500).unwrap();
    e.press(Button::Record);
    audio(&mut e, 20000, 0.);
    let s = e.digital_snapshot().unwrap();
    assert_eq!(s.state, 2);
    assert!((8000..10000).contains(&s.frames));
    assert!(s.eof);
    assert!(
        e.digital_samples()
            .unwrap()
            .iter()
            .filter(|v| **v > 0.29)
            .count()
            > 7900
    );
    assert_eq!(e.snapshot().written, 0);
}

#[test]
fn sequencer_edits_and_gates_leave_the_digital_loop_running_and_unchanged() {
    let mut e = boot();
    e.restore_digital(vec![0.25; 6250]).unwrap();
    e.set_digital_mode(true).unwrap();
    e.press(Button::Play);
    audio(&mut e, 4000, 0.);
    let saved = e.digital_samples().unwrap();
    e.sequence_mode(true, DeckId::Right).unwrap();
    e.press(Button::Record);
    audio(&mut e, 2000, 0.);
    assert_eq!(e.digital_snapshot().unwrap().state, 2);
    assert_eq!(e.digital_samples().unwrap(), saved);
    e.configure_sequence(3, &[1]).unwrap();
    audio(&mut e, 1000, 0.);
    e.test_instrument(false);
    e.press(Button::Play);
    let mut monitor = Vec::new();
    for _ in 0..80 {
        monitor.extend(e.render_monitor(&[0.; 256], 62500).unwrap());
    }
    assert!(monitor.iter().all(|v| (v[0] - v[1]).abs() < 0.000001));
    assert!(monitor.iter().any(|v| v[0] > 0.2));
    assert!(monitor.iter().any(|v| v[0] < 0.001));
    assert!(e.digital_snapshot().unwrap().wraps > 2);
    assert_eq!(e.digital_samples().unwrap(), saved);
    assert_eq!(e.snapshot().written, 0);
}
