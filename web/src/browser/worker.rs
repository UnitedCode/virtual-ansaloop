use super::*;
use virtual_ansaloop_engine::{Button, DeckId, Engine, EXPECTED_SHA};
struct Runtime {
    engine: Engine,
    scope: DedicatedWorkerGlobalScope,
    db: [Option<IdbDatabase>; 2],
    saving: [bool; 2],
    blocks: u64,
    levels: [f64; 2],
    level: f64,
    active: DeckId,
    source_names: [String; 3],
    digital: bool,
    digital_db: Option<IdbDatabase>,
    digital_saved: u32,
    digital_saving: bool,
    export_key: Option<(usize, u32, u64)>,
}
fn send(scope: &DedicatedWorkerGlobalScope, kind: &str, text: &str) {
    let o = packet(kind);
    set(&o, "text", text);
    let _ = scope.post_message(&o);
}
fn prepare_export(r: &mut Runtime) -> Result<(), JsValue> {
    if r.digital {
        let s = r.engine.digital_snapshot().map_err(js_error)?;
        let key = (2, s.frames, u64::from(s.revision));
        if s.capturing() || s.frames == 0 || r.export_key == Some(key) {
            return Ok(());
        }
        if !r.engine.prepare_digital_snapshot(0).map_err(js_error)? {
            return Ok(());
        }
        let bytes = Uint8Array::from(r.engine.export_digital_wav().map_err(js_error)?.as_slice());
        let o = packet("export");
        set(&o, "bytes", &bytes);
        set(&o, "seconds", f64::from(s.frames) / 62500.);
        set(&o, "deck", r.active.index() as u32);
        set(&o, "digital", true);
        r.scope
            .post_message_with_transfer(&o, &Array::of1(&bytes.buffer()))?;
        r.export_key = Some(key);
        return Ok(());
    }
    let s = r.engine.snapshot_deck(r.active);
    let key = (r.active.index(), s.tape_frames, s.written);
    if !s.can_swap || r.export_key == Some(key) {
        return Ok(());
    }
    let bytes = Uint8Array::from(
        r.engine
            .export_wav_deck(r.active)
            .map_err(js_error)?
            .as_slice(),
    );
    let o = packet("export");
    set(&o, "bytes", &bytes);
    set(&o, "seconds", s.seconds);
    set(&o, "deck", r.active.index() as u32);
    set(&o, "digital", false);
    r.scope
        .post_message_with_transfer(&o, &Array::of1(&bytes.buffer()))?;
    r.export_key = Some(key);
    Ok(())
}
fn deck_status(r: &Runtime, id: DeckId) -> Object {
    let s = r.engine.snapshot_deck(id);
    let o = Object::new();
    set(&o, "deck", id.index() as u32);
    set(&o, "state", s.state);
    set(&o, "seconds", s.seconds);
    set(&o, "position", s.position);
    set(&o, "gpio", s.gpio);
    set(&o, "canSwap", s.can_swap);
    set(&o, "frames", s.frames as f64);
    set(&o, "written", s.written as f64);
    set(
        &o,
        "level",
        if r.digital { 0. } else { r.levels[id.index()] },
    );
    set(&o, "sourcePosition", s.source_position);
    set(&o, "sourceDuration", s.source_duration);
    set(&o, "eof", s.eof);
    set(&o, "microphone", s.microphone);
    set(&o, "repeat", s.repeat);
    set(&o, "sourceName", r.source_names[id.index()].as_str());
    let catalog = Array::new();
    for n in r.engine.catalog_deck(id) {
        catalog.push(&n.into());
    }
    set(&o, "catalog", catalog);
    o
}
fn status(r: &mut Runtime, diagnostics: Option<&JsValue>) {
    let o = if r.digital {
        let o = Object::new();
        if let Ok(s) = r.engine.digital_snapshot() {
            set(&o, "state", s.name());
            set(&o, "seconds", f64::from(s.frames) / 62500.);
            set(&o, "position", f64::from(s.position) / 62500.);
            set(
                &o,
                "written",
                if s.state == 1 { s.position } else { s.frames },
            );
            set(&o, "revision", s.revision);
            set(&o, "wraps", s.wraps);
            set(&o, "canSwap", !s.capturing());
            set(
                &o,
                "exportReady",
                !s.capturing()
                    && s.frames > 0
                    && r.export_key == Some((2, s.frames, u64::from(s.revision))),
            );
            set(&o, "sourcePosition", s.source_position);
            set(&o, "sourceDuration", s.source_duration);
            set(&o, "microphone", s.microphone);
            set(&o, "repeat", s.repeat);
            set(&o, "eof", s.eof);
        }
        set(&o, "sourceName", r.source_names[2].as_str());
        set(&o, "deck", r.active.index() as u32);
        o
    } else {
        deck_status(r, r.active)
    };
    set(&o, "digital", r.digital);
    set(&o, "frames", r.engine.snapshot().frames as f64);
    set(&o, "kind", "status");
    set(&o, "sha", EXPECTED_SHA);
    set(&o, "mixLevel", r.level);
    let decks = Array::new();
    for id in DeckId::ALL {
        decks.push(&deck_status(r, id));
    }
    set(&o, "decks", decks);
    if let Ok(s) = r.engine.sequence_state() {
        let seq = Object::new();
        set(&seq, "running", s.running);
        set(&seq, "step", s.step as u32);
        set(&seq, "cursor", s.cursor as u32);
        set(&seq, "bpm", s.bpm);
        set(&seq, "target", s.target);
        set(&seq, "external", s.external);
        set(&seq, "overflow", s.overflow);
        let masks = Array::new();
        let notes = Array::new();
        for t in 0..2 {
            masks.push(&(s.masks[t] as u32).into());
            notes.push(&Uint8Array::from(s.notes[t].as_slice()));
        }
        set(&seq, "masks", masks);
        set(&seq, "notes", notes);
        set(&o, "sequence", seq);
    }
    if let Some(d) = diagnostics {
        for key in ["underruns", "recoveries", "queued", "captureDrops"] {
            set(&o, key, get(d, key));
        }
    }
    let _ = r.scope.post_message(&o);
}
fn save_if_stopped(runtime: Rc<RefCell<Runtime>>) {
    {
        let mut r = runtime.borrow_mut();
        if let Ok(s) = r.engine.digital_snapshot() {
            if !s.capturing()
                && s.revision != r.digital_saved
                && !r.digital_saving
                && r.engine.prepare_digital_snapshot(0).unwrap_or(false)
            {
                if let Some(db) = r.digital_db.clone() {
                    match r.engine.digital_samples() {
                        Ok(samples) => {
                            r.digital_saving = true;
                            drop(r);
                            let runtime = runtime.clone();
                            spawn_local(async move {
                                let result = storage::save_digital(&db, samples).await;
                                let mut r = runtime.borrow_mut();
                                r.digital_saving = false;
                                match result {
                                    Ok(()) => {
                                        r.digital_saved = s.revision;
                                        send(
                                            &r.scope,
                                            "storage",
                                            "Digital loop saved in this browser.",
                                        );
                                    }
                                    Err(e) => {
                                        r.digital_db = None;
                                        send(
                                            &r.scope,
                                            "warning",
                                            &format!("Digital save failed; export the loop: {e:?}"),
                                        );
                                    }
                                }
                            });
                        }
                        Err(e) => send(&r.scope, "warning", &e),
                    }
                }
            }
        }
    }
    for id in DeckId::ALL {
        let i = id.index();
        let mut r = runtime.borrow_mut();
        if r.saving[i] || !r.engine.dirty_decks[i] || !r.engine.snapshot_deck(id).can_swap {
            continue;
        }
        let Some(db) = r.db[i].clone() else {
            continue;
        };
        let tapes = r.engine.tapes_deck(id);
        let selected = r.engine.snapshot_deck(id).tape_frames;
        r.engine.dirty_decks[i] = false;
        r.saving[i] = true;
        drop(r);
        let runtime = runtime.clone();
        spawn_local(async move {
            let result = storage::save(&db, i, tapes, selected).await;
            let mut r = runtime.borrow_mut();
            r.saving[i] = false;
            match result {
                Ok(()) => send(&r.scope, "storage", "Stopped tapes saved in this browser."),
                Err(e) => {
                    r.engine.dirty_decks[i] = true;
                    send(&r.scope, "warning", &format!("{e:?}"));
                }
            }
        });
    }
}
fn audio(
    runtime: &Rc<RefCell<Runtime>>,
    port: &MessagePort,
    data: &JsValue,
) -> Result<(), JsValue> {
    let mic = Float32Array::new(&get(data, "mic")).to_vec();
    let mut r = runtime.borrow_mut();
    let tracks = r
        .engine
        .render_monitor(&mic, number(data, "hz") as u32)
        .map_err(js_error)?;
    let n = tracks.len().max(1) as f64;
    r.levels = std::array::from_fn(|i| {
        (tracks.iter().map(|v| f64::from(v[i]).powi(2)).sum::<f64>() / n).sqrt()
    });
    let output = tracks
        .iter()
        .map(|v| (v[0] + v[1]) * 0.5)
        .collect::<Vec<_>>();
    r.level = (output.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / n).sqrt();
    let o = packet("audio");
    let samples = Float32Array::from(output.as_slice());
    set(&o, "audio", &samples);
    port.post_message_with_transferable(&o, &Array::of1(&samples.buffer()))?;
    r.blocks += 1;
    r.engine.prepare_digital_snapshot(2048).map_err(js_error)?;
    let midi = r.engine.take_midi();
    if !midi.is_empty() {
        let o = packet("midi-out");
        let bytes = Uint8Array::from(midi.iter().map(|v| v.1).collect::<Vec<_>>().as_slice());
        set(&o, "bytes", bytes);
        let _ = r.scope.post_message(&o);
    }
    prepare_export(&mut r)?;
    if r.blocks.is_multiple_of(16) {
        status(&mut r, Some(data));
    }
    drop(r);
    save_if_stopped(runtime.clone());
    Ok(())
}
#[wasm_bindgen]
pub async fn start_worker() -> Result<(), JsValue> {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let mut engine = Engine::new().map_err(js_error)?;
    let opened = storage::open(&scope).await.ok();
    let mut db = [opened.clone(), opened.clone()];
    for id in DeckId::ALL {
        let i = id.index();
        let mut valid = true;
        if let Some(database) = &db[i] {
            match storage::load(database, i).await {
                Ok(library) => {
                    valid = library.selected.is_none_or(|selected| {
                        library.tapes.iter().any(|(frames, _)| *frames == selected)
                    });
                    for (frames, samples) in library.tapes {
                        if let Err(e) = engine.restore_frames_deck(id, frames, samples) {
                            valid = false;
                            send(&scope, "warning", &e);
                        }
                    }
                    if valid {
                        if let Some(frames) = library.selected {
                            if let Err(e) = engine.insert_tape_deck(id, f64::from(frames) / 62500.)
                            {
                                valid = false;
                                send(&scope, "warning", &e);
                            }
                        }
                    }
                }
                Err(_) => valid = false,
            }
        }
        if !valid {
            db[i] = None;
            send(&scope,"storage","A deck's saved tapes could not be loaded completely. Its original records were retained; export new recordings as WAV.");
        }
    }
    if db.iter().all(Option::is_none) {
        send(
            &scope,
            "storage",
            "Browser saving unavailable. Export recordings as WAV before closing.",
        );
    }
    engine.dirty = false;
    engine.dirty_decks = [false; 2];
    engine.render(&[0.; 256], 62500).map_err(js_error)?;
    let mut digital_db = opened.clone();
    if let Some(database) = &digital_db {
        let result = match storage::load_digital(database).await {
            Ok(Some(samples)) => engine.restore_digital(samples).map_err(js_error),
            Ok(None) => Ok(()),
            Err(e) => Err(e),
        };
        if result.is_err() {
            digital_db = None;
            send(&scope,"storage","Saved digital loop could not be loaded. Its original record was retained; export new loops as WAV.");
        }
    }
    let digital_saved = engine.digital_snapshot().map_err(js_error)?.revision;
    let runtime = Rc::new(RefCell::new(Runtime {
        engine,
        scope: scope.clone(),
        db,
        saving: [false; 2],
        blocks: 0,
        levels: [0.; 2],
        level: 0.,
        active: DeckId::Right,
        source_names: Default::default(),
        digital: false,
        digital_db,
        digital_saved,
        digital_saving: false,
        export_key: None,
    }));
    let state = runtime.clone();
    let callback = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let data = event.data();
        let result = (|| -> Result<(), JsValue> {
            if string(&data, "kind") == "connect" {
                let port = get(&data, "port").dyn_into::<MessagePort>()?;
                let r = state.clone();
                let p = port.clone();
                let handler = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                    if let Err(error) = audio(&r, &p, &e.data()) {
                        send(&r.borrow().scope, "error", &format!("{error:?}"));
                    }
                });
                port.set_onmessage(Some(handler.as_ref().unchecked_ref()));
                handler.forget();
                port.start();
                return Ok(());
            }
            let mut r = state.borrow_mut();
            let id = DeckId::from_index(number(&data, "deck") as usize).map_err(js_error)?;
            let digital = get(&data, "digital").as_bool().unwrap_or(false);
            match string(&data, "kind").as_str() {
                "audio-mode" => {
                    r.digital = digital;
                    r.engine.set_digital_mode(digital).map_err(js_error)?;
                    r.export_key = None;
                }
                "digital-clear" => r.engine.clear_digital().map_err(js_error)?,
                "select" => r.active = id,
                "record" | "play" | "stop" => {
                    r.engine
                        .sequence_mode(get(&data, "sequenceMode").as_bool().unwrap_or(false), id)
                        .map_err(js_error)?;
                    let b = match string(&data, "kind").as_str() {
                        "record" => Button::Record,
                        "play" => Button::Play,
                        _ => Button::Stop,
                    };
                    r.engine.press_deck(id, b);
                }
                "sequence-mode" => r
                    .engine
                    .sequence_mode(get(&data, "sequenceMode").as_bool().unwrap_or(false), id)
                    .map_err(js_error)?,
                "sequence-cue" => r.engine.cue_button().map_err(js_error)?,
                "sequence-config" => r
                    .engine
                    .configure_sequence(
                        number(&data, "command") as u8,
                        &Uint8Array::new(&get(&data, "args")).to_vec(),
                    )
                    .map_err(js_error)?,
                "midi-in" => r
                    .engine
                    .midi_input(&Uint8Array::new(&get(&data, "bytes")).to_vec())
                    .map_err(js_error)?,
                "test-instrument" => r
                    .engine
                    .test_instrument(get(&data, "enabled").as_bool().unwrap_or(false)),
                "play-both" => {
                    if r.digital {
                        return Err(js_error("Switch to tape mode first"));
                    }
                    r.engine.sequence_mode(false, id).map_err(js_error)?;
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Play);
                    }
                }
                "stop-both" => {
                    if r.digital {
                        return Err(js_error("Switch to tape mode first"));
                    }
                    r.engine.sequence_mode(false, id).map_err(js_error)?;
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Stop);
                    }
                }
                "swap" => r
                    .engine
                    .insert_tape_deck(id, number(&data, "seconds"))
                    .map_err(js_error)?,
                "source" => {
                    if digital {
                        r.engine
                            .load_digital_source(
                                Float32Array::new(&get(&data, "samples")).to_vec(),
                                number(&data, "hz") as u32,
                            )
                            .map_err(js_error)?;
                    } else {
                        r.engine
                            .load_source_deck(
                                id,
                                Float32Array::new(&get(&data, "samples")).to_vec(),
                                number(&data, "hz") as u32,
                            )
                            .map_err(js_error)?;
                    }
                    r.source_names[if digital { 2 } else { id.index() }] = string(&data, "name");
                    send(&r.scope, "notice", "Audio loaded.");
                }
                "mic" if digital => r
                    .engine
                    .set_digital_microphone(get(&data, "enabled").as_bool().unwrap_or(false)),
                "repeat" if digital => r
                    .engine
                    .set_digital_repeat(get(&data, "enabled").as_bool().unwrap_or(false)),
                "cue" if digital => r.engine.rewind_digital_source(),
                "mic" => r
                    .engine
                    .set_microphone(id, get(&data, "enabled").as_bool().unwrap_or(false)),
                "repeat" => r
                    .engine
                    .set_repeat(id, get(&data, "enabled").as_bool().unwrap_or(false)),
                "cue" => r.engine.rewind_source_deck(id),
                "export" => r.export_key = None,
                "shutdown" => {
                    r.engine.sequence_mode(true, id).map_err(js_error)?;
                    r.engine.press_deck(id, Button::Stop);
                    r.engine.render(&[0.; 2048], 62500).map_err(js_error)?;
                    r.engine.sequence_mode(false, id).map_err(js_error)?;
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Stop);
                    }
                    for _ in 0..80 {
                        r.engine.render(&[0.; 256], 62500).map_err(js_error)?;
                    }
                    let digital = r
                        .digital_db
                        .clone()
                        .map(|db| r.engine.digital_samples().map(|samples| (db, samples)))
                        .transpose()
                        .map_err(js_error)?;
                    let libraries = DeckId::ALL.map(|id| {
                        (
                            r.db[id.index()].clone(),
                            r.engine.tapes_deck(id),
                            r.engine.snapshot_deck(id).tape_frames,
                        )
                    });
                    let scope = r.scope.clone();
                    drop(r);
                    spawn_local(async move {
                        for (i, (db, tapes, selected)) in libraries.into_iter().enumerate() {
                            if let Some(db) = db {
                                if storage::save(&db, i, tapes, selected).await.is_err() {
                                    send(&scope,"warning","Tape save failed. Keep this page open and export the recording.");
                                    return;
                                }
                            }
                        }
                        if let Some((db, samples)) = digital {
                            if storage::save_digital(&db, samples).await.is_err() {
                                send(
                                    &scope,
                                    "warning",
                                    "Digital save failed. Keep this page open and export the loop.",
                                );
                                return;
                            }
                        }
                        send(&scope, "off", "");
                    });
                    return Ok(());
                }
                _ => return Err(js_error("unknown interface message")),
            }
            prepare_export(&mut r)?;
            status(&mut r, None);
            drop(r);
            save_if_stopped(state.clone());
            Ok(())
        })();
        if let Err(e) = result {
            send(&state.borrow().scope, "warning", &format!("{e:?}"));
        }
    });
    scope.set_onmessage(Some(callback.as_ref().unchecked_ref()));
    callback.forget();
    status(&mut runtime.borrow_mut(), None);
    send(&scope, "ready", EXPECTED_SHA);
    Ok(())
}
