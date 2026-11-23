use js_sys::{Array, Float32Array, Object, Reflect, Uint8Array};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::*;
mod storage;
mod ui;
mod worker;
pub use ui::start_ui;
pub use worker::start_worker;

fn get(o: &JsValue, k: &str) -> JsValue {
    Reflect::get(o, &k.into()).unwrap_or(JsValue::UNDEFINED)
}
fn set(o: &JsValue, k: &str, value: impl Into<JsValue>) {
    Reflect::set(o, &k.into(), &value.into()).unwrap();
}
fn packet(kind: &str) -> Object {
    let o = Object::new();
    set(&o, "kind", kind);
    o
}
fn number(o: &JsValue, k: &str) -> f64 {
    get(o, k).as_f64().unwrap_or(0.)
}
fn string(o: &JsValue, k: &str) -> String {
    get(o, k).as_string().unwrap_or_default()
}
fn js_error(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}
fn bind(target: &EventTarget, event: &str, callback: impl FnMut(Event) + 'static) {
    let f = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback(event, f.as_ref().unchecked_ref())
        .unwrap();
    f.forget();
}
