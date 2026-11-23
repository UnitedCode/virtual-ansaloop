use super::*;
use virtual_ansaloop_engine::{EXPECTED_SHA, MAX_SOURCE_FRAMES};

#[derive(Default)]
struct Ui {
    worker: Option<Worker>,
    context: Option<AudioContext>,
    node: Option<AudioWorkletNode>,
    stream: Option<MediaStream>,
    source: Option<MediaStreamAudioSourceNode>,
    ready: bool,
    active: usize,
    mic_decks: [bool; 2],
    mic_pending: bool,
    power_pending: bool,
    volume: f32,
    gain: Option<GainNode>,
    download_url: Option<String>,
    tape_seconds: f64,
    catalog: Vec<f64>,
}
fn doc() -> Document {
    window().unwrap().document().unwrap()
}
fn el(id: &str) -> Element {
    doc().get_element_by_id(id).unwrap()
}
fn text(id: &str, value: &str) {
    el(id).set_text_content(Some(value));
}
fn disabled(id: &str, value: bool) {
    let _ = el(id).set_attribute("aria-disabled", if value { "true" } else { "false" });
    if value {
        let _ = el(id).set_attribute("disabled", "");
    } else {
        let _ = el(id).remove_attribute("disabled");
    }
}
fn input(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}
fn front_position(value: f64) {
    let value = value.round().clamp(0., 2.);
    input("front-selector").set_value(&value.to_string());
    let _ = el("selector-assembly")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property("--position", &value.to_string());
    let _ = el("front-selector").set_attribute("data-position", &(value as u32 + 1).to_string());
    text(
        "notice",
        &format!(
            "Front selector: position {} (unassigned).",
            value as u32 + 1
        ),
    );
}
fn send(state: &Rc<RefCell<Ui>>, kind: &str) {
    if let Some(w) = &state.borrow().worker {
        let o = packet(kind);
        set(&o, "deck", state.borrow().active as u32);
        let _ = w.post_message(&o);
    }
}
fn warning(error: JsValue) {
    text(
        "notice",
        &error.as_string().unwrap_or_else(|| format!("{error:?}")),
    );
    web_sys::console::warn_1(&error);
}
fn stop_mic(state: &mut Ui) {
    state.mic_decks = [false; 2];
    if let Some(stream) = state.stream.take() {
        for track in stream.get_tracks().iter() {
            track.unchecked_into::<MediaStreamTrack>().stop();
        }
    }
    if let Some(source) = state.source.take() {
        let _ = source.disconnect();
    }
    text("mic", "Use microphone");
}
fn off(state: &Rc<RefCell<Ui>>) {
    let mut s = state.borrow_mut();
    s.ready = false;
    s.power_pending = false;
    stop_mic(&mut s);
    if let Some(node) = s.node.take() {
        let _ = node.disconnect();
    }
    if let Some(gain) = s.gain.take() {
        let _ = gain.disconnect();
    }
    if let Some(url) = s.download_url.take() {
        let _ = Url::revoke_object_url(&url);
    }
    let _ = el("export").remove_attribute("href");
    if let Some(ctx) = s.context.take() {
        let _ = ctx.close();
    }
    if let Some(worker) = s.worker.take() {
        worker.terminate();
    }
    for id in [
        "record",
        "stop",
        "play",
        "audio-file",
        "mic",
        "mic-jack",
        "cue",
        "repeat",
        "export",
        "tape6",
        "tape8",
        "primary-slot",
        "secondary-slot",
        "deck-left",
        "deck-right",
        "play-both",
        "stop-both",
        "insert-tape",
        "tape-library",
    ] {
        disabled(id, true);
    }
    disabled("power", false);
    disabled("power-jack", false);
    let _ = el("power-jack").set_attribute("aria-pressed", "false");
    for id in ["power-led", "record-led", "play-led"] {
        let _ = el(id).class_list().remove_1("on");
    }
    text("power", "Power on");
    let _ = el("power").class_list().remove_1("on");
    text("transport-state", "POWER OFF");
    for attribute in [
        "data-gpio",
        "data-written",
        "data-frames",
        "data-output-level",
    ] {
        let _ = el("transport-state").remove_attribute(attribute);
    }
    let _ = el("output-level")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property("width", "0%");
    text("swap-note", "Power on before inserting a tape.");
    text("device-note", "Power on to begin.");
    for id in ["deck-left-state", "deck-right-state"] {
        text(id, "POWER OFF");
    }
    text("source-name", "No file loaded");
    text("source-description", "Drag an audio file here.");
    text("source-clock", "");
    input("repeat").set_checked(false);
    let _ = el("source-progress-bar")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property("width", "0%");
    let _ = el("transport-led").set_attribute("class", "led");
    for id in ["record", "stop", "play"] {
        let _ = el(id).class_list().remove_1("active");
    }
}
fn update_status(state: &Rc<RefCell<Ui>>, data: &JsValue) {
    let decks = Array::from(&get(data, "decks"));
    for (i, d) in decks.iter().enumerate() {
        state.borrow_mut().mic_decks[i] = get(&d, "microphone").as_bool().unwrap_or(false);
        let side = if i == 0 { "right" } else { "left" };
        text(
            &format!("deck-{side}-state"),
            &format!(
                "{} · {:.2}s / {}s",
                string(&d, "state").to_uppercase(),
                number(&d, "position"),
                number(&d, "seconds")
            ),
        );
        let card = el(&format!("deck-{side}"));
        let _ = card.set_attribute("data-gpio", &format!("{}", number(&d, "gpio") as u16));
        let _ = card.set_attribute("data-written", &format!("{}", number(&d, "written") as u64));
        let _ = card.set_attribute("data-output-level", &number(&d, "level").to_string());
        let _ = card.set_attribute(
            "aria-pressed",
            if state.borrow().active == i {
                "true"
            } else {
                "false"
            },
        );
        let _ = card.set_attribute("data-position", &number(&d, "position").to_string());
        text(
            if i == 0 {
                "cassette-number"
            } else {
                "cassette-number-left"
            },
            &format!("{}s", number(&d, "seconds")),
        );
        for reel in if i == 0 {
            ["reel-left", "reel-right"]
        } else {
            ["reel-left-b", "reel-right-b"]
        } {
            let _ = el(reel)
                .unchecked_into::<HtmlElement>()
                .style()
                .set_property(
                    "transform",
                    &format!("rotate({}deg)", number(&d, "position") * 180.),
                );
        }
    }
    for (id, on) in [
        (
            "record-led",
            decks.iter().any(|d| string(&d, "state") == "Recording"),
        ),
        (
            "play-led",
            decks.iter().any(|d| string(&d, "state") == "Playing"),
        ),
    ] {
        if on {
            let _ = el(id).class_list().add_1("on");
        } else {
            let _ = el(id).class_list().remove_1("on");
        }
    }
    if number(data, "deck") as usize != state.borrow().active {
        return;
    }
    let side = if state.borrow().active == 0 {
        "Right"
    } else {
        "Left"
    };
    text("selected-deck", &format!("{side} deck"));
    let name = string(data, "sourceName");
    text(
        "source-name",
        if name.is_empty() {
            "No file loaded"
        } else {
            &name
        },
    );
    text(
        "source-description",
        if name.is_empty() {
            "Drag an audio file here."
        } else {
            ""
        },
    );
    input("repeat").set_checked(get(data, "repeat").as_bool().unwrap_or(false));
    text(
        "mic",
        if get(data, "microphone").as_bool() == Some(true) {
            "Microphone on"
        } else {
            "Use microphone"
        },
    );
    let snapshot = string(data, "state");
    let seconds = number(data, "seconds");
    let changed = state.borrow().tape_seconds != seconds;
    state.borrow_mut().tape_seconds = seconds;
    if changed {
        input("custom-length").set_value(&seconds.to_string());
    }
    let catalog = Array::from(&get(data, "catalog"))
        .iter()
        .filter_map(|v| v.as_f64())
        .collect::<Vec<_>>();
    if state.borrow().catalog != catalog {
        let select = el("tape-library");
        select.set_text_content(None);
        for seconds in &catalog {
            let option = doc().create_element("option").unwrap();
            option.set_attribute("value", &seconds.to_string()).unwrap();
            option.set_text_content(Some(&format!("{seconds}s")));
            select.append_child(&option).unwrap();
        }
        state.borrow_mut().catalog = catalog;
    }
    el("tape-library")
        .unchecked_into::<HtmlSelectElement>()
        .set_value(&seconds.to_string());
    let can_swap = get(data, "canSwap").as_bool().unwrap_or(false);
    text("transport-state", &snapshot.to_uppercase());
    let position = number(data, "position");
    text("counter", &format!("{position:05.2}"));
    text("tape-length", &format!("{seconds} SEC"));
    {
        let list = el("power-led").class_list();
        if state.borrow().ready {
            let _ = list.add_1("on");
        } else {
            let _ = list.remove_1("on");
        }
    }
    disabled("insert-tape", !can_swap);
    disabled("tape-library", !can_swap);
    let _ = el("transport-led").set_attribute(
        "class",
        if snapshot == "Recording" {
            "led rec"
        } else {
            "led lit"
        },
    );
    for (id, active) in [
        ("record", snapshot == "Recording"),
        ("stop", snapshot == "Stopped"),
        ("play", snapshot == "Playing"),
    ] {
        let list = el(id).class_list();
        if active {
            let _ = list.add_1("active");
        } else {
            let _ = list.remove_1("active");
        }
    }
    for (id, length) in [("tape6", 6.), ("tape8", 8.)] {
        disabled(id, !can_swap);
        let _ = el(id).set_attribute(
            "aria-pressed",
            if seconds == length { "true" } else { "false" },
        );
        let list = el(id).class_list();
        if seconds == length {
            let _ = list.add_1("selected");
        } else {
            let _ = list.remove_1("selected");
        }
    }
    text(
        "swap-note",
        if can_swap {
            "Ready to swap tapes."
        } else {
            "Stop before swapping."
        },
    );
    text(
        "device-note",
        match snapshot.as_str() {
            "Recording" => "Recording replaces existing tape audio.",
            "Playing" => "",
            "Starting" => "Motor starting.",
            "Stopping" => "Motor stopping.",
            _ => "",
        },
    );
    let source_pos = number(data, "sourcePosition");
    let duration = number(data, "sourceDuration");
    let mic = get(data, "microphone").as_bool().unwrap_or(false);
    if mic {
        text(
            "source-clock",
            "Live microphone · use headphones while recording.",
        );
    } else if duration > 0. {
        text(
            "source-clock",
            &format!(
                "{source_pos:.1} / {duration:.1} seconds{}",
                if get(data, "eof").as_bool() == Some(true) {
                    " · Input finished"
                } else {
                    ""
                }
            ),
        );
    } else {
        text("source-clock", "");
    }
    let _ = el("source-progress-bar")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property(
            "width",
            &format!(
                "{}%",
                if duration > 0. {
                    source_pos / duration * 100.
                } else {
                    0.
                }
            ),
        );
    let drops =
        number(data, "underruns") + number(data, "recoveries") + number(data, "captureDrops");
    text("engine-status",&format!("Actual ARM image {}… · GPIO {:04b} · {} audio frames · {} stream recoveries. Fast linear tape model; 62,500 Hz firmware codec.",&EXPECTED_SHA[..16],number(data,"gpio") as u16,number(data,"frames") as u64,drops as u64));
    // Inspectable DOM evidence follows the firmware, not a desired UI state.
    let _ = el("transport-state")
        .set_attribute("data-gpio", &format!("{}", number(data, "gpio") as u16));
    let _ = el("transport-state").set_attribute(
        "data-written",
        &format!("{}", number(data, "written") as u64),
    );
    let _ = el("transport-state")
        .set_attribute("data-frames", &format!("{}", number(data, "frames") as u64));
    let level = number(data, "mixLevel");
    let _ = el("transport-state").set_attribute("data-output-level", &format!("{level}"));
    let _ = el("output-level")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property("width", &format!("{}%", (level * 300.).clamp(0., 100.)));
    if state.borrow().ready {
        disabled("export", !can_swap);
    }
}
fn connect_audio(state: &Rc<RefCell<Ui>>) -> Result<(), JsValue> {
    let s = state.borrow();
    let ctx = s
        .context
        .as_ref()
        .ok_or_else(|| js_error("audio unavailable"))?;
    let options = AudioWorkletNodeOptions::new();
    options.set_number_of_inputs(1);
    options.set_number_of_outputs(1);
    options.set_output_channel_count(&Array::of1(&2.into()));
    let node = AudioWorkletNode::new_with_options(ctx, "ansaloop-audio", &options)?;
    let channel = MessageChannel::new()?;
    let o = packet("connect");
    set(&o, "port", channel.port1());
    s.worker
        .as_ref()
        .unwrap()
        .post_message_with_transfer(&o, &Array::of1(&channel.port1()))?;
    let o = packet("connect");
    set(&o, "connect", channel.port2());
    node.port()?
        .post_message_with_transferable(&o, &Array::of1(&channel.port2()))?;
    let gain = ctx.create_gain()?;
    gain.gain().set_value(s.volume);
    node.connect_with_audio_node(&gain)?;
    gain.connect_with_audio_node(&ctx.destination())?;
    drop(s);
    let mut s = state.borrow_mut();
    s.node = Some(node);
    s.gain = Some(gain);
    s.ready = true;
    s.power_pending = false;
    for id in [
        "record",
        "stop",
        "play",
        "audio-file",
        "mic",
        "mic-jack",
        "cue",
        "repeat",
        "export",
        "tape6",
        "tape8",
        "insert-tape",
        "tape-library",
        "primary-slot",
        "secondary-slot",
        "deck-left",
        "deck-right",
        "play-both",
        "stop-both",
    ] {
        disabled(id, false);
    }
    disabled("power", false);
    disabled("power-jack", false);
    let _ = el("power-jack").set_attribute("aria-pressed", "true");
    text("power", "Power off");
    let _ = el("power").class_list().add_1("on");
    let o = packet("select");
    set(&o, "deck", s.active as u32);
    let _ = s.worker.as_ref().unwrap().post_message(&o);
    text("notice", "");
    Ok(())
}
async fn power_on(state: Rc<RefCell<Ui>>) -> Result<(), JsValue> {
    let ctx = AudioContext::new()?;
    let resume = ctx.resume()?;
    state.borrow_mut().context = Some(ctx.clone());
    JsFuture::from(resume).await?;
    JsFuture::from(ctx.audio_worklet()?.add_module("audio-worklet.js")?).await?;
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    let worker = Worker::new_with_options("worker.js", &options)?;
    let s = state.clone();
    let message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let data = event.data();
        match string(&data, "kind").as_str() {
            "ready" => {
                if let Err(e) = connect_audio(&s) {
                    warning(e);
                    off(&s);
                }
            }
            "status" => update_status(&s, &data),
            "notice" => text("notice", &string(&data, "text")),
            "storage" => text("storage-status", &string(&data, "text")),
            "warning" => {
                text("notice", &string(&data, "text"));
                let ready = s.borrow().ready;
                if ready {
                    s.borrow_mut().power_pending = false;
                    disabled("power", false);
                    disabled("power-jack", false);
                }
            }
            "error" => {
                warning(js_error(string(&data, "text")));
                off(&s);
            }
            "off" => off(&s),
            "export" => {
                if number(&data, "deck") as usize != s.borrow().active {
                    return;
                }
                let bytes = Uint8Array::new(&get(&data, "bytes"));
                let properties = BlobPropertyBag::new();
                properties.set_type("audio/wav");
                match Blob::new_with_u8_array_sequence_and_options(&Array::of1(&bytes), &properties)
                    .and_then(|blob| Url::create_object_url_with_blob(&blob))
                {
                    Ok(url) => {
                        let anchor: HtmlAnchorElement = el("export").unchecked_into();
                        anchor.set_href(&url);
                        anchor.set_download(&format!(
                            "ansaloop-{}-{}s.wav",
                            if number(&data, "deck") == 0. {
                                "right"
                            } else {
                                "left"
                            },
                            number(&data, "seconds")
                        ));
                        if let Some(old) = s.borrow_mut().download_url.replace(url) {
                            let release = Closure::<dyn FnMut()>::once(move || {
                                let _ = Url::revoke_object_url(&old);
                            });
                            let _ = window()
                                .unwrap()
                                .set_timeout_with_callback_and_timeout_and_arguments_0(
                                    release.as_ref().unchecked_ref(),
                                    10000,
                                );
                            release.forget();
                        }
                    }
                    Err(e) => warning(e),
                }
            }
            _ => {}
        }
    });
    worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
    message.forget();
    let s = state.clone();
    let error = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        warning(js_error(format!(
            "Could not run the instrument: {:?}",
            get(e.as_ref(), "message")
        )));
        off(&s);
    });
    worker.set_onerror(Some(error.as_ref().unchecked_ref()));
    error.forget();
    state.borrow_mut().worker = Some(worker);
    Ok(())
}
async fn import_file(state: Rc<RefCell<Ui>>, file: File) -> Result<(), JsValue> {
    let deck = state.borrow().active;
    if file.size() > 100_000_000. {
        return Err(js_error("Choose an audio file under 100 MB."));
    }
    let ctx = state
        .borrow()
        .context
        .clone()
        .ok_or_else(|| js_error("Power on before loading audio."))?;
    text("notice", "Decoding your audio locally…");
    let bytes = JsFuture::from(file.array_buffer())
        .await?
        .dyn_into::<js_sys::ArrayBuffer>()?;
    let buffer = JsFuture::from(ctx.decode_audio_data(&bytes)?)
        .await?
        .dyn_into::<AudioBuffer>()?;
    if buffer.length() as usize > MAX_SOURCE_FRAMES {
        return Err(js_error(
            "Audio exceeds the browser's input-memory budget. Use a shorter clip or live input.",
        ));
    }
    let mut mono = vec![0f32; buffer.length() as usize];
    for channel in 0..buffer.number_of_channels() {
        let data = buffer.get_channel_data(channel)?;
        for (out, value) in mono.iter_mut().zip(data) {
            *out += value / buffer.number_of_channels() as f32;
        }
    }
    for value in &mut mono {
        if !value.is_finite() {
            return Err(js_error("The audio contains invalid samples."));
        }
        *value = value.clamp(-1., 1.);
    }
    let samples = Float32Array::from(mono.as_slice());
    let o = packet("source");
    set(&o, "deck", deck as u32);
    set(&o, "name", file.name());
    set(&o, "samples", &samples);
    set(&o, "hz", buffer.sample_rate());
    let mut s = state.borrow_mut();
    if s.context.as_ref() != Some(&ctx) {
        return Err(js_error("Instrument restarted while decoding."));
    }
    let worker = s
        .worker
        .clone()
        .ok_or_else(|| js_error("Instrument powered off while decoding."))?;
    s.mic_decks[deck] = false;
    if !s.mic_decks.iter().any(|v| *v) {
        stop_mic(&mut s);
    }
    worker.post_message_with_transfer(&o, &Array::of1(&samples.buffer()))?;
    Ok(())
}
async fn microphone(state: Rc<RefCell<Ui>>) -> Result<(), JsValue> {
    let deck = state.borrow().active;
    let existing = state.borrow().stream.is_some();
    if state.borrow().mic_decks[deck] {
        let mut s = state.borrow_mut();
        s.mic_decks[deck] = false;
        if !s.mic_decks.iter().any(|v| *v) {
            stop_mic(&mut s);
        }
        let o = packet("mic");
        set(&o, "deck", deck as u32);
        set(&o, "enabled", false);
        s.worker.as_ref().unwrap().post_message(&o)?;
        return Ok(());
    }
    if existing {
        let mut s = state.borrow_mut();
        s.mic_decks[deck] = true;
        let o = packet("mic");
        set(&o, "deck", deck as u32);
        set(&o, "enabled", true);
        s.worker.as_ref().unwrap().post_message(&o)?;
        return Ok(());
    }
    let constraints = MediaStreamConstraints::new();
    let audio = Object::new();
    for key in ["echoCancellation", "noiseSuppression", "autoGainControl"] {
        set(&audio, key, false);
    }
    constraints.set_audio(&audio);
    constraints.set_video(&JsValue::FALSE);
    let stream = JsFuture::from(
        window()
            .unwrap()
            .navigator()
            .media_devices()?
            .get_user_media_with_constraints(&constraints)?,
    )
    .await?
    .dyn_into::<MediaStream>()?;
    let mut s = state.borrow_mut();
    if !s.ready {
        for track in stream.get_tracks().iter() {
            track.unchecked_into::<MediaStreamTrack>().stop();
        }
        return Err(js_error("Power on before enabling the microphone."));
    }
    let source = s
        .context
        .as_ref()
        .unwrap()
        .create_media_stream_source(&stream)?;
    source.connect_with_audio_node(s.node.as_ref().unwrap())?;
    s.stream = Some(stream);
    s.source = Some(source);
    s.mic_decks[deck] = true;
    let o = packet("mic");
    set(&o, "deck", deck as u32);
    set(&o, "enabled", true);
    s.worker.as_ref().unwrap().post_message(&o)?;
    text("mic", "Microphone on");
    text(
        "notice",
        "Microphone ready. Use headphones, then press Record.",
    );
    Ok(())
}
#[wasm_bindgen]
pub fn start_ui() {
    bind(el("show-led-pcb").as_ref(), "change", move |_| {
        let list = el("led-module").class_list();
        if input("show-led-pcb").checked() {
            let _ = list.add_1("pcb-view");
        } else {
            let _ = list.remove_1("pcb-view");
        }
    });
    let state = Rc::new(RefCell::new(Ui {
        volume: 0.75,
        tape_seconds: 6.,
        ..Ui::default()
    }));
    disabled("power", false);
    disabled("power-jack", false);
    text("power", "Power on");
    text(
        "engine-status",
        &format!(
            "Verified ARM firmware {}… · Power on to boot Virtual Daisy.",
            &EXPECTED_SHA[..16]
        ),
    );
    for id in ["power", "power-jack"] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| {
            if s.borrow().power_pending {
                return;
            }
            s.borrow_mut().power_pending = true;
            disabled("power-jack", true);
            if s.borrow().worker.is_some() {
                disabled("power", true);
                text("power", "Saving tapes…");
                send(&s, "shutdown");
                return;
            }
            disabled("power", true);
            text("power", "Starting…");
            let state = s.clone();
            spawn_local(async move {
                if let Err(e) = power_on(state.clone()).await {
                    warning(e);
                    off(&state);
                }
            });
        });
    }
    for (id, kind) in [
        ("record", "record"),
        ("stop", "stop"),
        ("play", "play"),
        ("cue", "cue"),
        ("play-both", "play-both"),
        ("stop-both", "stop-both"),
    ] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| {
            text("notice", "");
            send(&s, kind);
        });
    }
    for (id, seconds) in [("tape6", 6), ("tape8", 8)] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| {
            if let Some(w) = &s.borrow().worker {
                let o = packet("swap");
                set(&o, "deck", s.borrow().active as u32);
                set(&o, "seconds", seconds);
                let _ = w.post_message(&o);
            }
        });
    }
    let s = state.clone();
    bind(el("repeat").as_ref(), "change", move |_| {
        if let Some(w) = &s.borrow().worker {
            let o = packet("repeat");
            set(&o, "deck", s.borrow().active as u32);
            set(&o, "enabled", input("repeat").checked());
            let _ = w.post_message(&o);
        }
    });
    let s = state.clone();
    bind(el("audio-file").as_ref(), "change", move |_| {
        if let Some(file) = input("audio-file").files().and_then(|f| f.item(0)) {
            let state = s.clone();
            spawn_local(async move {
                if let Err(e) = import_file(state, file).await {
                    warning(e);
                }
            });
        }
        input("audio-file").set_value("");
    });
    for id in ["mic", "mic-jack"] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| {
            if s.borrow().mic_pending {
                return;
            }
            s.borrow_mut().mic_pending = true;
            disabled("mic", true);
            let state = s.clone();
            spawn_local(async move {
                let result = microphone(state.clone()).await;
                state.borrow_mut().mic_pending = false;
                disabled("mic", !state.borrow().ready);
                if let Err(e) = result {
                    warning(e);
                }
            });
        });
    }
    let s = state.clone();
    bind(el("volume").as_ref(), "input", move |_| {
        let volume = input("volume").value_as_number().clamp(0., 1.) as f32;
        let mut s = s.borrow_mut();
        s.volume = volume;
        if let Some(gain) = &s.gain {
            gain.gain().set_value(volume);
        }
    });
    for (id, name) in [
        ("side-selector", "Side three-position switch"),
        ("side-binary", "Side two-position switch"),
    ] {
        bind(el(id).as_ref(), "change", move |_| {
            let control = input(id);
            let value = if id == "side-binary" {
                u32::from(control.checked()) + 1
            } else {
                control.value_as_number() as u32 + 1
            };
            let _ = control.set_attribute("data-position", &value.to_string());
            text("notice", &format!("{name}: position {value} (unassigned)."));
        });
    }
    bind(el("midi-jack").as_ref(), "click", move |_| {
        text(
            "notice",
            "MIDI input is planned. This tape firmware does not use MIDI yet.",
        )
    });
    for (id, deck) in [
        ("primary-slot", 0),
        ("secondary-slot", 1),
        ("deck-right", 0),
        ("deck-left", 1),
    ] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| {
            {
                let mut ui = s.borrow_mut();
                ui.active = deck;
                ui.tape_seconds = f64::NAN;
                ui.catalog.clear();
            }
            disabled("export", true);
            let _ = el("export").remove_attribute("href");
            send(&s, "select");
        });
    }
    let s = state.clone();
    bind(el("insert-tape").as_ref(), "click", move |_| {
        if let Some(worker) = &s.borrow().worker {
            let o = packet("swap");
            set(&o, "deck", s.borrow().active as u32);
            set(&o, "seconds", input("custom-length").value_as_number());
            let _ = worker.post_message(&o);
        }
    });
    let s = state.clone();
    bind(el("tape-library").as_ref(), "change", move |_| {
        let select: HtmlSelectElement = el("tape-library").unchecked_into();
        if let Ok(seconds) = select.value().parse::<f64>() {
            if let Some(worker) = &s.borrow().worker {
                let o = packet("swap");
                set(&o, "deck", s.borrow().active as u32);
                set(&o, "seconds", seconds);
                let _ = worker.post_message(&o);
            }
        }
    });
    bind(el("front-selector").as_ref(), "input", move |_| {
        front_position(input("front-selector").value_as_number())
    });
    let drag = Rc::new(RefCell::new(None::<(i32, f64)>));
    let d = drag.clone();
    bind(el("front-selector").as_ref(), "pointerdown", move |e| {
        e.prevent_default();
        let p: PointerEvent = e.unchecked_into();
        let control = el("front-selector");
        let _ = input("front-selector").focus();
        let _ = control.set_pointer_capture(p.pointer_id());
        *d.borrow_mut() = Some((p.client_x(), input("front-selector").value_as_number()));
    });
    let d = drag.clone();
    bind(el("front-selector").as_ref(), "pointermove", move |e| {
        if let Some((start, value)) = *d.borrow() {
            e.prevent_default();
            let p: PointerEvent = e.unchecked_into();
            let scale = el("device-view").get_bounding_client_rect().width() / 1230.;
            front_position(value + f64::from(p.client_x() - start) / (22. * scale).max(0.1));
        }
    });
    for event in ["pointerup", "pointercancel", "lostpointercapture"] {
        let d = drag.clone();
        bind(el("front-selector").as_ref(), event, move |_| {
            *d.borrow_mut() = None;
        });
    }
    let s = state.clone();
    bind(el("export").as_ref(), "click", move |e| {
        if el("export").has_attribute("disabled") || !el("export").has_attribute("href") {
            e.prevent_default();
            if s.borrow().ready {
                send(&s, "export");
            }
        }
    });
    bind(el("drop").as_ref(), "dragover", move |e| {
        e.prevent_default();
        let _ = el("drop").class_list().add_1("drag");
    });
    bind(el("drop").as_ref(), "dragleave", move |_| {
        let _ = el("drop").class_list().remove_1("drag");
    });
    let s = state.clone();
    bind(el("drop").as_ref(), "drop", move |e| {
        e.prevent_default();
        let _ = el("drop").class_list().remove_1("drag");
        let event: DragEvent = e.unchecked_into();
        if let Some(file) = event
            .data_transfer()
            .and_then(|d| d.files())
            .and_then(|f| f.item(0))
        {
            let state = s.clone();
            spawn_local(async move {
                if let Err(e) = import_file(state, file).await {
                    warning(e);
                }
            });
        }
    });
    let s = state.clone();
    bind(doc().as_ref(), "keydown", move |e| {
        let event: KeyboardEvent = e.unchecked_into();
        if event.repeat()
            || event.ctrl_key()
            || event.meta_key()
            || event.alt_key()
            || !s.borrow().ready
        {
            return;
        }
        if let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) {
            if ["INPUT", "TEXTAREA", "SELECT"].contains(&target.tag_name().as_str())
                || target.get_attribute("contenteditable").is_some()
            {
                return;
            }
        }
        let kind = match event.key().to_lowercase().as_str() {
            "r" => "record",
            "s" => "stop",
            " " => "play",
            _ => return,
        };
        event.prevent_default();
        send(&s, kind);
    });
}
