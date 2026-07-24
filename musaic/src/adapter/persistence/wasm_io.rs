//! Browser save (download) and open (hidden file input) for `.musaic.json` projects.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Blob, BlobPropertyBag, FileReader, HtmlAnchorElement, Url};

use crate::application::session::MusaicProject;

use super::{PersistenceError, export_project_bytes, import_project_bytes};

const RECENT_STORAGE_KEY: &str = "musaic_recent_projects";

static OPEN_QUEUE: OnceLock<Mutex<Vec<Result<MusaicProject, String>>>> = OnceLock::new();

fn open_queue() -> &'static Mutex<Vec<Result<MusaicProject, String>>> {
    OPEN_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Resource, Debug, Default)]
pub struct WasmProjectIo {
    /// Drained by menu/editor systems after the file picker completes.
    pub pending_opens: Vec<Result<MusaicProject, String>>,
    pub pick_requested: bool,
}

pub struct WasmIoPlugin;

impl Plugin for WasmIoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WasmProjectIo>()
            .add_systems(Update, drain_wasm_open_queue);
    }
}

fn drain_wasm_open_queue(mut io: ResMut<WasmProjectIo>) {
    if let Ok(mut queue) = open_queue().lock() {
        io.pending_opens.append(&mut queue);
    }
}

pub fn request_open_project() {
    if let Err(error) = open_file_picker() {
        bevy::log::error!("wasm open picker failed: {error}");
    }
}

pub fn download_project(project: &MusaicProject, filename: &str) -> Result<(), PersistenceError> {
    let bytes = export_project_bytes(project)?;
    trigger_download(filename, &bytes)
        .map_err(|e| PersistenceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
}

fn trigger_download(filename: &str, bytes: &[u8]) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let blob = Blob::new_with_blob_sequence_and_options(
        &parts,
        BlobPropertyBag::new().type_("application/json"),
    )
    .map_err(|_| "blob failed")?;
    let url = Url::create_object_url_with_blob(&blob).map_err(|_| "object url failed")?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "anchor failed")?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "anchor cast failed")?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    Url::revoke_object_url(&url).ok();
    Ok(())
}

fn open_file_picker() -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let input = document
        .create_element("input")
        .map_err(|_| "input failed")?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| "input cast failed")?;
    input.set_type("file");
    input.set_accept(".json,application/json");

    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(files) = input.files() else {
            return;
        };
        if files.length() == 0 {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };
        let reader = FileReader::new().expect("FileReader");
        let reader_for_closure = reader.clone();
        let on_load = Closure::wrap(Box::new(move |_evt: web_sys::Event| {
            let result = reader_for_closure
                .result()
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::ArrayBuffer>().ok())
                .map(|buf| {
                    let array = js_sys::Uint8Array::new(&buf);
                    let mut bytes = vec![0u8; array.length() as usize];
                    array.copy_to(&mut bytes);
                    bytes
                });
            let parsed = match result {
                Some(bytes) => import_project_bytes(&bytes, None).map_err(|e| e.to_string()),
                None => Err("failed to read file".into()),
            };
            if let Ok(mut queue) = open_queue().lock() {
                queue.push(parsed);
            }
        }) as Box<dyn FnMut(_)>);
        reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
        on_load.forget();
        let _ = reader.read_as_array_buffer(&file);
    }) as Box<dyn FnMut(_)>);
    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();
    input.click();
    Ok(())
}

pub fn load_recent_projects() -> Vec<PathBuf> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return Vec::new(),
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return Vec::new(),
    };
    let Ok(text) = storage.get_item(RECENT_STORAGE_KEY) else {
        return Vec::new();
    };
    let Some(text) = text else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn push_recent_project(path: impl AsRef<std::path::Path>) {
    let path = path.as_ref().to_path_buf();
    let mut recent = load_recent_projects();
    recent.retain(|p| p != &path);
    recent.insert(0, path);
    recent.truncate(12);
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(json) = serde_json::to_string(&recent) {
                let _ = storage.set_item(RECENT_STORAGE_KEY, &json);
            }
        }
    }
}
