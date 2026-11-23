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
    source_names: [String; 2],
    export_key: Option<(usize, u32, u64)>,
}
fn send(scope: &DedicatedWorkerGlobalScope, kind: &str, text: &str) {
    let o = packet(kind);
    set(&o, "text", text);
    let _ = scope.post_message(&o);
}
fn prepare_export(r: &mut Runtime) -> Result<(), JsValue> {
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
    set(&o, "level", r.levels[id.index()]);
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
fn status(r: &Runtime, diagnostics: Option<&JsValue>) {
    let o = deck_status(r, r.active);
    set(&o, "kind", "status");
    set(&o, "sha", EXPECTED_SHA);
    set(&o, "mixLevel", r.level);
    let decks = Array::new();
    for id in DeckId::ALL {
        decks.push(&deck_status(r, id));
    }
    set(&o, "decks", decks);
    if let Some(d) = diagnostics {
        for key in ["underruns", "recoveries", "queued", "captureDrops"] {
            set(&o, key, get(d, key));
        }
    }
    let _ = r.scope.post_message(&o);
}
fn save_if_stopped(runtime: Rc<RefCell<Runtime>>) {
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
        .render_tracks(&mic, number(data, "hz") as u32)
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
    prepare_export(&mut r)?;
    if r.blocks.is_multiple_of(16) {
        status(&r, Some(data));
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
    let mut db = [opened.clone(), opened];
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
            match string(&data, "kind").as_str() {
                "select" => r.active = id,
                "record" => r.engine.press_deck(id, Button::Record),
                "play" => r.engine.press_deck(id, Button::Play),
                "stop" => r.engine.press_deck(id, Button::Stop),
                "play-both" => {
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Play);
                    }
                }
                "stop-both" => {
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Stop);
                    }
                }
                "swap" => r
                    .engine
                    .insert_tape_deck(id, number(&data, "seconds"))
                    .map_err(js_error)?,
                "source" => {
                    r.engine
                        .load_source_deck(
                            id,
                            Float32Array::new(&get(&data, "samples")).to_vec(),
                            number(&data, "hz") as u32,
                        )
                        .map_err(js_error)?;
                    r.source_names[id.index()] = string(&data, "name");
                    send(&r.scope, "notice", "Audio loaded.");
                }
                "mic" => r
                    .engine
                    .set_microphone(id, get(&data, "enabled").as_bool().unwrap_or(false)),
                "repeat" => r
                    .engine
                    .set_repeat(id, get(&data, "enabled").as_bool().unwrap_or(false)),
                "cue" => r.engine.rewind_source_deck(id),
                "export" => r.export_key = None,
                "shutdown" => {
                    for id in DeckId::ALL {
                        r.engine.press_deck(id, Button::Stop);
                    }
                    for _ in 0..80 {
                        r.engine.render(&[0.; 256], 62500).map_err(js_error)?;
                    }
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
                        send(&scope, "off", "");
                    });
                    return Ok(());
                }
                _ => return Err(js_error("unknown interface message")),
            }
            prepare_export(&mut r)?;
            status(&r, None);
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
    status(&runtime.borrow(), None);
    send(&scope, "ready", EXPECTED_SHA);
    Ok(())
}
