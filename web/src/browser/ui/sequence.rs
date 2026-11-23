// Loaded as ui::sequence. Rust owns the editor and Web MIDI platform bridge.
use super::*;
#[derive(Default)]
pub(super) struct View {
    cursor: usize,
    masks: [u16; 2],
    notes: [[u8; 16]; 2],
    access: Option<JsValue>,
    midi_in: Option<JsValue>,
    midi_out: Option<JsValue>,
    status: u8,
    pending: Vec<u8>,
    clocks: u64,
    messages: u64,
}
fn config(state: &Rc<RefCell<Ui>>, command: u8, args: &[u8]) {
    if let Some(worker) = &state.borrow().worker {
        let o = packet("sequence-config");
        set(&o, "deck", state.borrow().active as u32);
        set(&o, "command", command);
        set(&o, "args", Uint8Array::from(args));
        let _ = worker.post_message(&o);
    }
}
fn midi_in(state: &Rc<RefCell<Ui>>, bytes: &[u8]) {
    if let Some(worker) = &state.borrow().worker {
        let o = packet("midi-in");
        set(&o, "deck", state.borrow().active as u32);
        set(&o, "bytes", Uint8Array::from(bytes));
        let _ = worker.post_message(&o);
    }
}
fn edit(state: &Rc<RefCell<Ui>>) {
    let s = state.borrow();
    let note = input("sequence-note").value_as_number();
    if !note.is_finite() || !(0. ..=127.).contains(&note) {
        return;
    }
    config(
        state,
        2,
        &[
            s.active as u8,
            s.sequence.cursor as u8,
            note.round() as u8,
            u8::from(input("sequence-enabled").checked()),
        ],
    );
}
pub(super) fn update(state: &Rc<RefCell<Ui>>, data: &JsValue) {
    if data.is_undefined() {
        return;
    }
    let running = get(data, "running").as_bool().unwrap_or(false);
    let step = number(data, "step") as usize;
    let cursor = (number(data, "cursor") as usize).min(15);
    let masks = Array::from(&get(data, "masks"));
    let notes = Array::from(&get(data, "notes"));
    let mut s = state.borrow_mut();
    s.sequence.cursor = cursor;
    for t in 0..2 {
        s.sequence.masks[t] = masks.get(t as u32).as_f64().unwrap_or(0.) as u16;
        let n = Uint8Array::new(&notes.get(t as u32)).to_vec();
        if n.len() == 16 {
            s.sequence.notes[t].copy_from_slice(&n);
        }
    }
    let track = s.active;
    let mask = s.sequence.masks[track];
    for i in 0..16 {
        let on = mask & (1 << i) != 0;
        let class = format!(
            "{}{}{}",
            if on { " enabled" } else { "" },
            if running && step == i { " playing" } else { "" },
            if cursor == i { " cursor" } else { "" }
        );
        let _ = el(&format!("step-{i}")).set_attribute("class", &class);
        let _ = el(&format!("step-{i}"))
            .set_attribute("aria-pressed", if on { "true" } else { "false" });
        let _ = el(&format!("led-{i}")).set_attribute("class", &format!("sequence-lamp{class}"));
    }
    text(
        "sequence-position",
        &if running {
            format!("STEP {} / 16", step + 1)
        } else {
            "STOPPED".into()
        },
    );
    let _ = el("sequence-position")
        .set_attribute("data-running", if running { "true" } else { "false" });
    let _ = el("sequence-position").set_attribute("data-step", &step.to_string());
    text("sequence-cursor", &(cursor + 1).to_string());
    let focused = doc()
        .active_element()
        .and_then(|e| e.get_attribute("id"))
        .unwrap_or_default();
    if focused != "sequence-note" {
        input("sequence-note").set_value(&s.sequence.notes[track][cursor].to_string());
    }
    input("sequence-enabled").set_checked(mask & (1 << cursor) != 0);
    if focused != "sequence-bpm" {
        input("sequence-bpm").set_value(&number(data, "bpm").to_string());
    }
    el("sequence-target")
        .unchecked_into::<HtmlSelectElement>()
        .set_value(&number(data, "target").to_string());
    input("sequence-clock").set_checked(get(data, "external").as_bool().unwrap_or(false));
    if number(data, "overflow") > 0. {
        text(
            "notice",
            "MIDI transmit queue overflow. Stop and restart the prototype.",
        );
    }
}
fn call(object: &JsValue, name: &str, args: &Array) -> Result<JsValue, JsValue> {
    get(object, name)
        .dyn_into::<js_sys::Function>()?
        .apply(object, args)
}
fn send_port(port: &JsValue, bytes: &[u8]) -> Result<(), JsValue> {
    call(port, "send", &Array::of1(&Uint8Array::from(bytes)))?;
    Ok(())
}
fn silence(s: &mut Ui) {
    if let Some(port) = &s.sequence.midi_out {
        let _ = send_port(port, &[0xfc, 0xb0, 123, 0, 0xb1, 123, 0]);
    }
}
pub(super) fn power(state: &Rc<RefCell<Ui>>, on: bool) {
    for id in [
        "sequence-cue",
        "sequence-mode",
        "sequence-bpm",
        "sequence-target",
        "sequence-note",
        "sequence-enabled",
        "sequence-clock",
        "test-instrument",
        "midi-test",
    ] {
        disabled(id, !on);
    }
    for i in 0..16 {
        disabled(&format!("step-{i}"), !on);
    }
    if !on {
        let mut s = state.borrow_mut();
        silence(&mut s);
        for i in 0..16 {
            let _ = el(&format!("led-{i}")).set_attribute("class", "sequence-lamp");
        }
        text("sequence-position", "POWER OFF");
    } else {
        let o = packet("test-instrument");
        set(&o, "deck", state.borrow().active as u32);
        set(&o, "enabled", input("test-instrument").checked());
        if let Some(w) = &state.borrow().worker {
            let _ = w.post_message(&o);
        }
    }
}
pub(super) fn midi_out(state: &Rc<RefCell<Ui>>, bytes: &[u8]) {
    let mut s = state.borrow_mut();
    let v = &mut s.sequence;
    for &b in bytes {
        let message = if b >= 0xf8 {
            Some(vec![b])
        } else if b & 128 != 0 {
            v.status = b;
            v.pending = vec![b];
            None
        } else {
            v.pending.push(b);
            let len = if matches!(v.status & 0xf0, 0xc0 | 0xd0) {
                2
            } else {
                3
            };
            if v.pending.len() == len {
                let msg = v.pending.clone();
                v.pending.clear();
                Some(msg)
            } else {
                None
            }
        };
        if let Some(message) = message {
            if b == 0xf8 {
                v.clocks += 1;
            } else if message.len() == 3 {
                v.messages += 1;
                text(
                    "midi-monitor",
                    &format!(
                        "OUT ch {} · {} {} · {} notes · {} clocks",
                        (message[0] & 15) + 1,
                        if message[0] & 0xf0 == 0x90 {
                            "ON"
                        } else {
                            "OFF"
                        },
                        message[1],
                        v.messages,
                        v.clocks
                    ),
                );
            }
            if let Some(port) = &v.midi_out {
                if let Err(e) = send_port(port, &message) {
                    text("midi-status", &format!("MIDI output unavailable: {e:?}"));
                }
            }
        }
    }
}
fn refresh_ports(state: &Rc<RefCell<Ui>>) -> Result<(), JsValue> {
    let access = state
        .borrow()
        .sequence
        .access
        .clone()
        .ok_or_else(|| js_error("MIDI unavailable"))?;
    for (id, name) in [("midi-in", "inputs"), ("midi-out", "outputs")] {
        let select = el(id).unchecked_into::<HtmlSelectElement>();
        let selected = select.value();
        select.set_text_content(None);
        let option = doc().create_element("option")?;
        option.set_attribute("value", "")?;
        option.set_text_content(Some("None"));
        select.append_child(&option)?;
        let values = call(&get(&access, name), "values", &Array::new())?;
        if let Some(iter) = js_sys::try_iter(&values)? {
            for port in iter {
                let port = port?;
                if string(&port, "state") == "disconnected" {
                    continue;
                }
                let option = doc().create_element("option")?;
                option.set_attribute("value", &string(&port, "id"))?;
                option.set_text_content(Some(&string(&port, "name")));
                select.append_child(&option)?;
            }
        }
        select.set_value(&selected);
    }
    Ok(())
}
fn choose_port(state: &Rc<RefCell<Ui>>, is_input: bool) -> Result<(), JsValue> {
    let access = state
        .borrow()
        .sequence
        .access
        .clone()
        .ok_or_else(|| js_error("Connect MIDI devices first"))?;
    let (id, name) = if is_input {
        ("midi-in", "inputs")
    } else {
        ("midi-out", "outputs")
    };
    let selected = el(id).unchecked_into::<HtmlSelectElement>().value();
    let port = if selected.is_empty() {
        None
    } else {
        let p = call(&get(&access, name), "get", &Array::of1(&selected.into()))?;
        if p.is_undefined() {
            None
        } else {
            Some(p)
        }
    };
    if is_input {
        if let Some(old) = state.borrow_mut().sequence.midi_in.take() {
            set(&old, "onmidimessage", JsValue::NULL);
        }
        if let Some(p) = &port {
            let s = state.clone();
            let callback = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
                let bytes = Uint8Array::new(&get(e.as_ref(), "data")).to_vec();
                if s.borrow().ready {
                    midi_in(&s, &bytes);
                    text("midi-status", &format!("IN · {} MIDI bytes", bytes.len()));
                }
            });
            set(p, "onmidimessage", callback.as_ref());
            callback.forget();
        }
        state.borrow_mut().sequence.midi_in = port;
    } else {
        let mut s = state.borrow_mut();
        silence(&mut s);
        s.sequence.midi_out = port;
    }
    text(
        "midi-status",
        "Selected MIDI ports updated. None disconnects a port.",
    );
    Ok(())
}
pub(super) fn init(state: &Rc<RefCell<Ui>>) {
    for i in 0..16 {
        let button = doc().create_element("button").unwrap();
        button.set_id(&format!("step-{i}"));
        button.set_text_content(Some(&(i + 1).to_string()));
        button
            .set_attribute("aria-label", &format!("Edit sequencer step {}", i + 1))
            .unwrap();
        el("step-editor").append_child(&button).unwrap();
        let lamp = doc().create_element("i").unwrap();
        lamp.set_id(&format!("led-{i}"));
        lamp.set_attribute("class", "sequence-lamp").unwrap();
        lamp.set_attribute(
            "style",
            &format!("left:{}%", (13. + i as f64 * 8.) / 150. * 100.),
        )
        .unwrap();
        el("sequence-leds").append_child(&lamp).unwrap();
        let s = state.clone();
        bind(button.as_ref(), "click", move |_| {
            config(&s, 5, &[i as u8]);
        });
    }
    for id in ["sequence-note", "sequence-enabled"] {
        let s = state.clone();
        let event = if id == "sequence-note" {
            "input"
        } else {
            "change"
        };
        bind(el(id).as_ref(), event, move |_| edit(&s));
    }
    let s = state.clone();
    bind(el("sequence-bpm").as_ref(), "input", move |_| {
        let bpm = input("sequence-bpm").value_as_number();
        if bpm.is_finite() && (30. ..=240.).contains(&bpm) {
            let b = bpm.round() as u16;
            config(&s, 1, &[(b & 127) as u8, (b >> 7) as u8]);
        }
    });
    let s = state.clone();
    bind(el("sequence-target").as_ref(), "change", move |_| {
        let n = el("sequence-target")
            .unchecked_into::<HtmlSelectElement>()
            .value()
            .parse()
            .unwrap_or(0);
        config(&s, 3, &[n]);
    });
    let s = state.clone();
    bind(el("sequence-clock").as_ref(), "change", move |_| {
        config(&s, 4, &[u8::from(input("sequence-clock").checked())]);
    });
    let s = state.clone();
    bind(el("sequence-mode").as_ref(), "change", move |_| {
        let n = if input("sequence-mode").checked() {
            1.
        } else if s.borrow().active == 0 {
            2.
        } else {
            0.
        };
        front_position(&s, n);
    });
    let s = state.clone();
    bind(el("test-instrument").as_ref(), "change", move |_| {
        let o = packet("test-instrument");
        set(&o, "deck", s.borrow().active as u32);
        set(&o, "enabled", input("test-instrument").checked());
        if let Some(w) = &s.borrow().worker {
            let _ = w.post_message(&o);
        }
    });
    let s = state.clone();
    bind(el("midi-test").as_ref(), "click", move |_| {
        let n = input("midi-test-note").value_as_number();
        if n.is_finite() && (0. ..=127.).contains(&n) {
            midi_in(&s, &[0x90, n as u8, 100, 0x80, n as u8, 0]);
        }
    });
    let s = state.clone();
    bind(el("midi-connect").as_ref(), "click", move |_| {
        let s = s.clone();
        spawn_local(async move {
            let result = async {
                let navigator: JsValue = window().unwrap().navigator().into();
                if !get(&navigator, "requestMIDIAccess").is_function() {
                    return Err(js_error(
                        "Web MIDI is unavailable in this browser. The virtual demo still works.",
                    ));
                }
                let access = JsFuture::from(
                    call(&navigator, "requestMIDIAccess", &Array::new())?
                        .dyn_into::<js_sys::Promise>()?,
                )
                .await?;
                s.borrow_mut().sequence.access = Some(access.clone());
                refresh_ports(&s)?;
                let state = s.clone();
                let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
                    let _ = refresh_ports(&state);
                });
                set(&access, "onstatechange", callback.as_ref());
                callback.forget();
                text(
                    "midi-status",
                    "Choose an IN and/or OUT port. Ports start at None.",
                );
                Ok::<(), JsValue>(())
            }
            .await;
            if let Err(e) = result {
                text(
                    "midi-status",
                    &e.as_string().unwrap_or_else(|| format!("{e:?}")),
                );
            }
        });
    });
    for (id, is_input) in [("midi-in", true), ("midi-out", false)] {
        let s = state.clone();
        bind(el(id).as_ref(), "change", move |_| {
            if let Err(e) = choose_port(&s, is_input) {
                warning(e);
            }
        });
    }
    let _ = el("selector-assembly")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property("--position", "2");
    power(state, false);
}
