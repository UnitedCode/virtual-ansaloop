use super::*;
pub(super) fn select(state: &Rc<RefCell<Ui>>, enabled: bool) {
    state.borrow_mut().digital = enabled;
    input("side-binary").set_checked(enabled);
    for (id, on) in [("digital-mode", enabled), ("tape-mode", !enabled)] {
        let _ = el(id).set_attribute("aria-pressed", if on { "true" } else { "false" });
    }
    for (id, hidden) in [
        ("digital-panel", !enabled),
        ("tape-panel", enabled),
        ("deck-choice", enabled),
        ("both-transport", enabled),
    ] {
        if hidden {
            let _ = el(id).set_attribute("hidden", "");
        } else {
            let _ = el(id).remove_attribute("hidden");
        }
    }
    let _ = el("device-view")
        .class_list()
        .toggle_with_force("digital-mode", enabled);
    input("sequence-mode").set_checked(false);
    send(state, "audio-mode");
    send(state, "sequence-mode");
    disabled("export", true);
    let _ = el("export").remove_attribute("href");
    text(
        "selected-deck",
        if enabled {
            "Digital loop"
        } else {
            "Tape loops"
        },
    );
}
pub(super) fn status(state: &Rc<RefCell<Ui>>, data: &JsValue) {
    let name = string(data, "state");
    let seconds = number(data, "seconds");
    let position = number(data, "position");
    text(
        "digital-length",
        &if name == "Recording" {
            format!("{position:.2}s / 60s")
        } else if seconds > 0. {
            format!("{seconds:.2}s")
        } else {
            "EMPTY".into()
        },
    );
    let _ = el("digital-length").set_attribute("data-seconds", &seconds.to_string());
    let _ =
        el("digital-length").set_attribute("data-revision", &number(data, "revision").to_string());
    let _ = el("transport-state").set_attribute("data-digital", "true");
    let _ = el("digital-progress-bar")
        .unchecked_into::<HtmlElement>()
        .style()
        .set_property(
            "width",
            &format!(
                "{}%",
                (position
                    / if name == "Recording" {
                        60.
                    } else {
                        seconds.max(0.000016)
                    }
                    * 100.)
                    .clamp(0., 100.)
            ),
        );
    disabled(
        "digital-clear",
        !state.borrow().ready || !matches!(name.as_str(), "Stopped" | "Empty") || seconds == 0.,
    );
    let recording = matches!(name.as_str(), "Recording" | "Overdubbing");
    let _ = el("record-led")
        .class_list()
        .toggle_with_force("on", recording);
    let _ = el("play-led")
        .class_list()
        .toggle_with_force("on", matches!(name.as_str(), "Playing" | "Overdubbing"));
    text(
        "device-note",
        match name.as_str() {
            "Recording" => "PLAY closes the loop. STOP closes it without playing.",
            "Overdubbing" => "Adding to the loop. REC finishes this layer.",
            "Playing" => "REC adds a layer. STOP keeps the loop.",
            "Empty" => "Load audio or enable the microphone, then press REC.",
            _ => "PLAY resumes. REC starts a new recording.",
        },
    );
}
pub(super) fn init(state: &Rc<RefCell<Ui>>) {
    for (id, enabled) in [("digital-mode", true), ("tape-mode", false)] {
        let s = state.clone();
        bind(el(id).as_ref(), "click", move |_| select(&s, enabled));
    }
    let s = state.clone();
    bind(el("side-binary").as_ref(), "change", move |_| {
        select(&s, input("side-binary").checked())
    });
    let s = state.clone();
    bind(el("digital-clear").as_ref(), "click", move |_| {
        send(&s, "digital-clear")
    });
}
