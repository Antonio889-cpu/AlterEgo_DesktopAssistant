//! Piccola finestra con un modulo (caselle di testo, password, scelte) che Alter Ego apre quando le
//! serve un'informazione dall'utente, per esempio i dati di connessione a SQL Server.
//! I valori tornano al backend e non passano mai dal modello: le password restano qui.

use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

pub const LABEL: &str = "form";

#[derive(Default)]
pub struct Forms {
    pending: Mutex<Option<(u64, Value, oneshot::Sender<Option<Value>>)>>,
    next: AtomicU64,
    /// Modulo mostrato dalla finestra aperta (per non annullare quello nuovo quando si chiude la vecchia).
    shown: AtomicU64,
}

/// Campo del modulo. `kind`: "text", "password", "number", "select", "checkbox".
pub fn field(name: &str, label: &str, kind: &str, value: Value) -> Value {
    json!({ "name": name, "label": label, "type": kind, "value": value })
}

/// Apre il modulo e aspetta la risposta (5 minuti). None = annullato o finestra chiusa.
pub async fn ask(app: &AppHandle, title: &str, intro: &str, fields: Vec<Value>) -> Option<Value> {
    let forms = app.state::<Forms>();
    let id = forms.next.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = oneshot::channel();
    let spec = json!({ "id": id, "title": title, "intro": intro, "fields": fields });
    // un modulo alla volta: quello vecchio si annulla
    if let Some((_, _, old)) = forms.pending.lock().unwrap().replace((id, spec, tx)) {
        let _ = old.send(None);
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("form.html".into()))
        .title(title)
        .inner_size(540.0, 720.0)
        .min_inner_size(380.0, 300.0)
        .center()
        .always_on_top(true)
        .focused(true)
        .build();
    if let Err(e) = built {
        crate::applog::error("modulo", &format!("finestra non aperta: {e}"));
        forms.pending.lock().unwrap().take();
        return None;
    }
    let answer = tokio::time::timeout(Duration::from_secs(300), rx).await;
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
    let mut p = forms.pending.lock().unwrap();
    if p.as_ref().is_some_and(|(pid, _, _)| *pid == id) {
        p.take();
    }
    answer.ok().and_then(Result::ok).flatten()
}

/// Il modulo da mostrare (lo chiede form.html all'apertura).
#[tauri::command]
pub fn form_spec(app: AppHandle) -> Option<Value> {
    let forms = app.state::<Forms>();
    let p = forms.pending.lock().unwrap();
    let (id, spec, _) = p.as_ref()?;
    forms.shown.store(*id, Ordering::SeqCst);
    Some(spec.clone())
}

/// Risposta del modulo: `values` null = annullato.
#[tauri::command]
pub fn form_submit(app: AppHandle, id: u64, values: Option<Value>) {
    let forms = app.state::<Forms>();
    let mut p = forms.pending.lock().unwrap();
    if p.as_ref().is_some_and(|(pid, _, _)| *pid == id) {
        if let Some((_, _, tx)) = p.take() {
            let _ = tx.send(values);
        }
    }
}

/// Finestra chiusa con la X: annulla.
pub fn closed(app: &AppHandle) {
    let forms = app.state::<Forms>();
    let shown = forms.shown.swap(0, Ordering::SeqCst);
    let mut p = forms.pending.lock().unwrap();
    if p.as_ref().is_some_and(|(id, _, _)| *id == shown) {
        if let Some((_, _, tx)) = p.take() {
            let _ = tx.send(None);
        }
    }
}
