use virtual_ansaloop_engine::{Button, DeckId, Engine};
fn advance(e: &mut Engine, frames: usize) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    for offset in (0..frames).step_by(256) {
        out.extend(
            e.render_monitor(&vec![0.; (frames - offset).min(256)], 62500)
                .unwrap(),
        );
    }
    out
}
fn boot() -> Engine {
    let mut e = Engine::new().unwrap();
    advance(&mut e, 256);
    e
}
#[test]
fn real_arm_buttons_sequence_notes_and_paced_midi_clock() {
    let mut e = boot();
    e.sequence_mode(true, DeckId::Right).unwrap();
    e.cue_button().unwrap();
    advance(&mut e, 2000);
    assert_eq!(e.sequence_state().unwrap().cursor, 1);
    e.press(Button::Record);
    advance(&mut e, 2000);
    assert_eq!(e.sequence_state().unwrap().masks[0], 0x1113);
    assert_eq!(
        e.snapshot().written,
        0,
        "REC in sequencer mode cannot erase a tape"
    );
    e.press(Button::Play);
    let output = advance(&mut e, 62500);
    let s = e.sequence_state().unwrap();
    assert!(s.running);
    assert_eq!(s.overflow, 0);
    assert!(
        output.iter().any(|v| v[0].abs() > 0.05),
        "external MIDI receiver must be audible"
    );
    let bytes = e.take_midi();
    assert!(bytes.iter().any(|v| v.1 == 0xfa));
    let clocks = bytes
        .iter()
        .filter(|v| v.1 == 0xf8)
        .map(|v| v.0)
        .collect::<Vec<_>>();
    assert!((47..=48).contains(&clocks.len()), "{clocks:?}");
    for pair in clocks.windows(2) {
        assert!((20_750_000..20_920_000).contains(&(pair[1] - pair[0])));
    }
    let raw = bytes.iter().map(|v| v.1).collect::<Vec<_>>();
    assert!(raw.windows(3).any(|v| v == [0x90, 60, 100]));
    assert!(raw.windows(3).any(|v| v == [0x91, 48, 100]));
    e.press(Button::Stop);
    advance(&mut e, 3000);
    assert!(!e.sequence_state().unwrap().running);
    assert!(e.take_midi().iter().any(|v| v.1 == 0xfc));
}
#[test]
fn uart_editor_external_clock_and_note_entry_run_inside_arm() {
    let mut e = boot();
    e.sequence_mode(true, DeckId::Left).unwrap();
    advance(&mut e, 256);
    e.configure_sequence(1, &[100, 1]).unwrap(); // 228 BPM
    e.configure_sequence(2, &[1, 7, 67, 1]).unwrap();
    advance(&mut e, 1000);
    let s = e.sequence_state().unwrap();
    assert_eq!(s.bpm, 228);
    assert_eq!(s.notes[1][7], 67);
    assert_eq!(s.cursor, 7);
    e.midi_input(&[0x90, 72, 100]).unwrap();
    advance(&mut e, 1000);
    let s = e.sequence_state().unwrap();
    assert_eq!(s.notes[1][7], 72);
    assert_eq!(s.cursor, 8);
    e.configure_sequence(4, &[1]).unwrap();
    advance(&mut e, 1000);
    e.take_midi();
    e.midi_input(&[0xfa]).unwrap();
    advance(&mut e, 1000);
    assert!(e.sequence_state().unwrap().running);
    advance(&mut e, 10000);
    assert_eq!(
        e.sequence_state().unwrap().step,
        0,
        "host audio time cannot advance external-clock steps"
    );
    e.midi_input(&[0xf8; 6]).unwrap();
    advance(&mut e, 1000);
    assert_eq!(e.sequence_state().unwrap().step, 1);
    e.midi_input(&[0xfc]).unwrap();
    advance(&mut e, 1000);
    assert!(!e.sequence_state().unwrap().running);
}
#[test]
fn tape_gates_leave_motors_and_recordings_continuous() {
    let mut e = boot();
    for id in DeckId::ALL {
        e.restore_frames_deck(id, 62500, vec![0.3; 62500]).unwrap();
        e.insert_tape_deck(id, 1.).unwrap();
        e.press_deck(id, Button::Play);
    }
    advance(&mut e, 20000);
    let before = e.snapshot().position;
    e.configure_sequence(3, &[1]).unwrap();
    advance(&mut e, 1000);
    e.sequence_mode(true, DeckId::Right).unwrap();
    e.press(Button::Play);
    e.test_instrument(false);
    let out = advance(&mut e, 16000);
    assert_eq!(e.snapshot().gpio, 3);
    assert_eq!(e.snapshot_deck(DeckId::Left).gpio, 3);
    assert_ne!(e.snapshot().position, before);
    assert!(out.iter().any(|v| v[0].abs() > 0.2));
    assert!(out.iter().any(|v| v[0].abs() < 0.001));
    assert_eq!(
        e.tape_frames_deck(DeckId::Right, 62500).unwrap(),
        vec![0.3; 62500]
    );
}
