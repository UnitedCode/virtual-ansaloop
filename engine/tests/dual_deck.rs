use virtual_ansaloop_engine::{Button, DeckId, Engine};
const RIGHT: DeckId = DeckId::Right;
const LEFT: DeckId = DeckId::Left;
fn tone(hz: f64, seconds: f64) -> Vec<f32> {
    (0..(seconds * 48000.) as usize)
        .map(|i| (0.3 * (std::f64::consts::TAU * hz * i as f64 / 48000.).sin()) as f32)
        .collect()
}
fn render(e: &mut Engine, seconds: f64) -> Vec<[f32; 2]> {
    let count = (seconds * 48000.) as usize;
    let mut audio = Vec::new();
    for offset in (0..count).step_by(128) {
        audio.extend(
            e.render_tracks(&vec![0.; (count - offset).min(128)], 48000)
                .unwrap(),
        );
    }
    audio
}
fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
}
fn correlation(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum::<f64>()
        / (a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>()
            * b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>())
        .sqrt()
}
#[test]
fn one_arm_firmware_records_two_channels_and_plays_different_loops_simultaneously() {
    let mut e = Engine::new().unwrap();
    e.insert_tape_deck(RIGHT, 0.75).unwrap();
    e.insert_tape_deck(LEFT, 1.25).unwrap();
    e.load_source_deck(RIGHT, tone(221., 4.), 48000).unwrap();
    e.load_source_deck(LEFT, tone(703., 4.), 48000).unwrap();
    for id in DeckId::ALL {
        e.press_deck(id, Button::Record);
        assert_eq!(e.snapshot_deck(id).gpio, 0);
    }
    render(&mut e, 0.9);
    for id in DeckId::ALL {
        assert_eq!(e.snapshot_deck(id).gpio, 15);
        assert!(e.snapshot_deck(id).written > 40000);
    }
    e.press_deck(RIGHT, Button::Stop);
    render(&mut e, 0.3);
    assert_eq!(e.snapshot_deck(RIGHT).gpio, 0);
    assert_eq!(e.snapshot_deck(LEFT).gpio, 15);
    let right = e.tape_frames_deck(RIGHT, 46875).unwrap();
    let before = e.snapshot_deck(LEFT).written;
    render(&mut e, 0.5);
    assert!(e.snapshot_deck(LEFT).written > before + 30000);
    assert_eq!(e.tape_frames_deck(RIGHT, 46875).unwrap(), right);
    e.press_deck(LEFT, Button::Stop);
    render(&mut e, 0.3);
    let left = e.tape_frames_deck(LEFT, 78125).unwrap();
    assert!(rms(&right) > 0.18);
    assert!(rms(&left) > 0.18);
    for id in DeckId::ALL {
        e.press_deck(id, Button::Play);
    }
    let tracks = render(&mut e, 4.2);
    for id in DeckId::ALL {
        assert_eq!(e.snapshot_deck(id).gpio, 3);
    }
    let r = tracks.iter().skip(12000).map(|v| v[0]).collect::<Vec<_>>();
    let l = tracks.iter().skip(12000).map(|v| v[1]).collect::<Vec<_>>();
    assert!(rms(&r) > 0.18 && rms(&l) > 0.18);
    assert!(
        correlation(&r, &l).abs() < 0.05,
        "channels must retain different audio"
    );
    for (audio, lag) in [(&r, 36000), (&l, 60000)] {
        assert!(correlation(&audio[..audio.len() - lag], &audio[lag..]) > 0.999);
    }
    e.press_deck(RIGHT, Button::Stop);
    render(&mut e, 0.3);
    let position = e.snapshot_deck(LEFT).position;
    e.insert_tape_deck(RIGHT, 6.).unwrap();
    render(&mut e, 0.1);
    assert_eq!(e.snapshot_deck(LEFT).gpio, 3);
    assert_ne!(e.snapshot_deck(LEFT).position, position);
    assert_eq!(e.tape_frames_deck(LEFT, 78125).unwrap(), left);
    assert_eq!(e.tape_frames_deck(RIGHT, 46875).unwrap(), right);
    let (rx, tx) = e.audio_status().unwrap();
    assert_eq!((rx & 1, tx & 1), (0, 0));
}
#[test]
fn equal_length_cassettes_remain_distinct_across_two_deck_library_restore() {
    let mut e = Engine::new().unwrap();
    let right = tone(221., 1.);
    let left = tone(703., 1.);
    // 48,000 carrier frames correspond to 0.768 seconds.
    for (id, samples) in [(RIGHT, right.clone()), (LEFT, left.clone())] {
        e.restore_frames_deck(id, 48000, samples).unwrap();
        e.insert_tape_deck(id, 0.768).unwrap();
    }
    let mut restored = Engine::new().unwrap();
    for id in DeckId::ALL {
        for (frames, tape) in e.tapes_deck(id) {
            restored.restore_frames_deck(id, frames, tape).unwrap();
        }
        restored.insert_tape_deck(id, 0.768).unwrap();
    }
    assert_eq!(restored.tape_frames_deck(RIGHT, 48000).unwrap(), right);
    assert_eq!(restored.tape_frames_deck(LEFT, 48000).unwrap(), left);
    assert_ne!(
        restored.export_wav_deck(RIGHT).unwrap(),
        restored.export_wav_deck(LEFT).unwrap()
    );
}
#[test]
fn one_file_ending_stops_only_its_deck_while_the_other_repeats() {
    let mut e = Engine::new().unwrap();
    for id in DeckId::ALL {
        e.load_source_deck(id, tone(330., 0.2), 48000).unwrap();
        e.press_deck(id, Button::Record);
    }
    e.set_repeat(RIGHT, true);
    render(&mut e, 0.7);
    assert_eq!(e.snapshot_deck(LEFT).state, "Stopped");
    assert!(e.snapshot_deck(LEFT).eof);
    assert_eq!(e.snapshot_deck(RIGHT).state, "Recording");
    assert!(!e.snapshot_deck(RIGHT).eof);
}
