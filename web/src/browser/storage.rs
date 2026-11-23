//! Browser-local binary tape library, including migration of the original 6/8s tapes.
use super::*;
pub struct Library {
    pub tapes: Vec<(u32, Vec<f32>)>,
    pub selected: Option<u32>,
}
async fn request(request: &IdbRequest) -> Result<JsValue, JsValue> {
    let mut success = None;
    let mut failure = None;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let r = request.clone();
        let rejected = reject.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| match r.result() {
            Ok(value) => {
                let _ = resolve.call1(&JsValue::NULL, &value);
            }
            Err(e) => {
                let _ = rejected.call1(&JsValue::NULL, &e);
            }
        });
        request.set_onsuccess(Some(callback.as_ref().unchecked_ref()));
        success = Some(callback);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = reject.call1(&JsValue::NULL, &"browser tape storage unavailable".into());
        });
        request.set_onerror(Some(callback.as_ref().unchecked_ref()));
        failure = Some(callback);
    });
    let result = JsFuture::from(promise).await;
    request.set_onsuccess(None);
    request.set_onerror(None);
    drop(success);
    drop(failure);
    result
}
pub async fn open(scope: &DedicatedWorkerGlobalScope) -> Result<IdbDatabase, JsValue> {
    let factory = scope
        .indexed_db()?
        .ok_or_else(|| js_error("browser storage unavailable"))?;
    let open = factory.open_with_u32("virtual-ansaloop", 1)?;
    let r = open.clone();
    let upgrade = Closure::<dyn FnMut(Event)>::new(move |_| {
        if let Ok(db) = r.result().and_then(|v| v.dyn_into::<IdbDatabase>()) {
            let _ = db.create_object_store("tapes");
        }
    });
    open.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let result = request(open.unchecked_ref()).await;
    open.set_onupgradeneeded(None);
    drop(upgrade);
    result?.dyn_into()
}
async fn get_record(db: &IdbDatabase, key: JsValue) -> Result<JsValue, JsValue> {
    let tx = db.transaction_with_str("tapes")?;
    let req = tx.object_store("tapes")?.get(&key)?;
    request(&req).await
}
pub async fn load(db: &IdbDatabase) -> Result<Library, JsValue> {
    let catalog = get_record(db, "catalog-frames".into()).await?;
    let mut tapes = Vec::new();
    if catalog.is_undefined() {
        // Do not delete the legacy keys: migration keeps a recoverable copy.
        for seconds in [6u32, 8] {
            let value = get_record(db, seconds.into()).await?;
            if value.is_instance_of::<Float32Array>() {
                tapes.push((seconds * 62500, Float32Array::new(&value).to_vec()));
            }
        }
        return Ok(Library {
            tapes,
            selected: None,
        });
    }
    if !catalog.is_instance_of::<js_sys::Uint32Array>() {
        return Err(js_error("invalid saved tape catalog"));
    }
    for frames in js_sys::Uint32Array::new(&catalog).to_vec() {
        let value = get_record(db, format!("frames-{frames}").into()).await?;
        if !value.is_instance_of::<Float32Array>() {
            return Err(js_error("saved cassette is missing"));
        }
        tapes.push((frames, Float32Array::new(&value).to_vec()));
    }
    let selected = get_record(db, "inserted-frames".into())
        .await?
        .as_f64()
        .map(|n| n as u32);
    Ok(Library { tapes, selected })
}
pub async fn save(
    db: &IdbDatabase,
    tapes: Vec<(u32, Vec<f32>)>,
    selected: u32,
) -> Result<(), JsValue> {
    let tx = db.transaction_with_str_and_mode("tapes", IdbTransactionMode::Readwrite)?;
    let store = tx.object_store("tapes")?;
    let catalog = tapes.iter().map(|(frames, _)| *frames).collect::<Vec<_>>();
    for (frames, samples) in tapes {
        store.put_with_key(
            &Float32Array::from(samples.as_slice()),
            &format!("frames-{frames}").into(),
        )?;
    }
    store.put_with_key(
        &js_sys::Uint32Array::from(catalog.as_slice()),
        &"catalog-frames".into(),
    )?;
    store.put_with_key(&selected.into(), &"inserted-frames".into())?;
    let mut complete = None;
    let mut failure = None;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        tx.set_oncomplete(Some(callback.as_ref().unchecked_ref()));
        complete = Some(callback);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = reject.call1(
                &JsValue::NULL,
                &"could not save the tapes; export a WAV".into(),
            );
        });
        tx.set_onabort(Some(callback.as_ref().unchecked_ref()));
        failure = Some(callback);
    });
    let result = JsFuture::from(promise).await;
    tx.set_oncomplete(None);
    tx.set_onabort(None);
    drop(complete);
    drop(failure);
    result.map(|_| ())
}
