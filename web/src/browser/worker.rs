use super::*;
use virtual_ansaloop_engine::{Button, Engine, EXPECTED_SHA};

struct Runtime {
    engine: Engine,
    scope: DedicatedWorkerGlobalScope,
    db: Option<IdbDatabase>,
    saving: bool,
    blocks: u64,
    level: f64,
    export_key: Option<(u32, u64)>,
}
fn prepare_export(r: &mut Runtime) -> Result<(), JsValue> {
    let s = r.engine.snapshot();
    let key = (s.tape_frames, s.written);
    if !s.can_swap || r.export_key == Some(key) {
        return Ok(());
    }
    let bytes = Uint8Array::from(r.engine.export_wav().map_err(js_error)?.as_slice());
    let o = packet("export");
    set(&o, "bytes", &bytes);
    set(&o, "seconds", s.seconds);
    r.scope
        .post_message_with_transfer(&o, &Array::of1(&bytes.buffer()))?;
    r.export_key = Some(key);
    Ok(())
}
fn send(scope: &DedicatedWorkerGlobalScope, kind: &str, text: &str) {
    let o = packet(kind);
    set(&o, "text", text);
    let _ = scope.post_message(&o);
}
fn status(r: &Runtime, diagnostics: Option<&JsValue>) {
    let s = r.engine.snapshot();
    let o = packet("status");
    set(&o, "state", s.state);
    set(&o, "seconds", s.seconds);
    let catalog = Array::new();
    for seconds in r.engine.catalog() {
        catalog.push(&seconds.into());
    }
    set(&o, "catalog", catalog);
    set(&o, "position", s.position);
    set(&o, "speed", s.speed);
    set(&o, "gpio", s.gpio);
    set(&o, "canSwap", s.can_swap);
    set(&o, "frames", s.frames as f64);
    set(&o, "written", s.written as f64);
    set(&o, "level", r.level);
    set(&o, "sourcePosition", s.source_position);
    set(&o, "sourceDuration", s.source_duration);
    set(&o, "eof", s.eof);
    set(&o, "microphone", r.engine.microphone);
    set(&o, "sha", EXPECTED_SHA);
    if let Some(d) = diagnostics {
        for key in ["underruns", "recoveries", "queued", "captureDrops"] {
            set(&o, key, get(d, key));
        }
    }
    let _ = r.scope.post_message(&o);
}
fn save_if_stopped(runtime: Rc<RefCell<Runtime>>) {
    let mut r = runtime.borrow_mut();
    if r.saving || !r.engine.dirty || !r.engine.snapshot().can_swap {
        return;
    }
    let Some(db) = r.db.clone() else {
        return;
    };
    let tapes = r.engine.tapes();
    let selected = r.engine.snapshot().tape_frames;
    r.engine.dirty = false;
    r.saving = true;
    drop(r);
    spawn_local(async move {
        let result = storage::save(&db, tapes, selected).await;
        let mut r = runtime.borrow_mut();
        r.saving = false;
        match result {
            Ok(()) => send(&r.scope, "storage", "Tapes saved in this browser."),
            Err(e) => {
                r.engine.dirty = true;
                send(&r.scope, "warning", &format!("{e:?}"));
            }
        }
    });
}
fn audio(
    runtime: &Rc<RefCell<Runtime>>,
    port: &MessagePort,
    data: &JsValue,
) -> Result<(), JsValue> {
    let mic = Float32Array::new(&get(data, "mic")).to_vec();
    let mut r = runtime.borrow_mut();
    let output = r
        .engine
        .render(&mic, number(data, "hz") as u32)
        .map_err(js_error)?;
    r.level = (output.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>()
        / output.len().max(1) as f64)
        .sqrt();
    let o = packet("audio");
    let samples = Float32Array::from(output.as_slice());
    set(&o, "audio", &samples);
    port.post_message_with_transferable(&o, &Array::of1(&samples.buffer()))?;
    r.blocks += 1;
    if r.blocks.is_multiple_of(16) {
        status(&r, Some(data));
    }
    prepare_export(&mut r)?;
    drop(r);
    save_if_stopped(runtime.clone());
    Ok(())
}
#[wasm_bindgen]
pub async fn start_worker() -> Result<(), JsValue> {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let mut engine = Engine::new().map_err(js_error)?;
    let mut db = match storage::open(&scope).await {
        Ok(db) => Some(db),
        Err(_) => {
            send(
                &scope,
                "storage",
                "Browser storage unavailable. Keep this page open and export your tapes.",
            );
            None
        }
    };
    let mut storage_valid = true;
    if let Some(db) = &db {
        match storage::load(db).await {
            Ok(library) => {
                let selected_valid = library.selected.is_none_or(|selected| {
                    library.tapes.iter().any(|(frames, _)| *frames == selected)
                });
                storage_valid = selected_valid;
                for (frames, samples) in library.tapes {
                    if let Err(e) = engine.restore_frames(frames, samples) {
                        storage_valid = false;
                        send(&scope, "warning", &e);
                    }
                }
                if storage_valid {
                    if let Some(frames) = library.selected {
                        if let Err(e) = engine.insert_tape(f64::from(frames) / 62500.) {
                            storage_valid = false;
                            send(&scope, "warning", &e);
                        }
                    }
                }
                engine.dirty = false;
            }
            Err(_) => storage_valid = false,
        }
    }
    if !storage_valid {
        db = None;
        send(&scope,"storage","Saved tapes could not be loaded completely. Existing browser records were retained; automatic saving is disabled. Export new recordings as WAV.");
    }
    engine.render(&[0.; 256], 62500).map_err(js_error)?;
    let runtime = Rc::new(RefCell::new(Runtime {
        engine,
        scope: scope.clone(),
        db,
        saving: false,
        blocks: 0,
        level: 0.,
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
            match string(&data, "kind").as_str() {
                "record" => {
                    r.engine.press(Button::Record);
                }
                "play" => r.engine.press(Button::Play),
                "stop" => r.engine.press(Button::Stop),
                "swap" => r
                    .engine
                    .insert_tape(number(&data, "seconds"))
                    .map_err(js_error)?,
                "source" => {
                    r.engine
                        .load_source(
                            Float32Array::new(&get(&data, "samples")).to_vec(),
                            number(&data, "hz") as u32,
                        )
                        .map_err(js_error)?;
                    send(&r.scope, "notice", "Audio loaded.");
                }
                "mic" => r.engine.microphone = get(&data, "enabled").as_bool().unwrap_or(false),
                "repeat" => {
                    r.engine.repeat_source = get(&data, "enabled").as_bool().unwrap_or(false)
                }
                "cue" => r.engine.rewind_source(),
                "export" => {
                    let bytes =
                        Uint8Array::from(r.engine.export_wav().map_err(js_error)?.as_slice());
                    let o = packet("export");
                    set(&o, "bytes", &bytes);
                    set(&o, "seconds", r.engine.snapshot().seconds);
                    r.scope
                        .post_message_with_transfer(&o, &Array::of1(&bytes.buffer()))?;
                }
                "shutdown" => {
                    r.engine.press(Button::Stop);
                    for _ in 0..80 {
                        r.engine.render(&[0.; 256], 62500).map_err(js_error)?;
                    }
                    let tapes = r.engine.tapes();
                    let selected = r.engine.snapshot().tape_frames;
                    let db = r.db.clone();
                    let scope = r.scope.clone();
                    drop(r);
                    spawn_local(async move {
                        if let Some(db) = db {
                            if storage::save(&db, tapes, selected).await.is_err() {
                                send(&scope,"warning","Tape save failed. Keep this page open and export the recording.");
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
