use wasm_bindgen::prelude::*;

use super::{WorkspaceProjectSnapshot, canonical_project_name_seed};

const WORKSPACE_LOCATION_KEY: &str = "cadence.workspace.location";
const WORKSPACE_PAYLOAD_KEY: &str = "cadence.workspace.payload";
const RECOVERY_PAYLOAD_KEY: &str = "cadence.recovery.payload";
const IMPORT_PREFIX: &str = "import://";
const WORKSPACE_PREFIX: &str = "browser://workspace/";
const DOWNLOAD_PREFIX: &str = "download://";

#[wasm_bindgen(inline_js = r#"
const cadenceImports = new Map();
const cadenceStorageCache = new Map();
const cadenceStoragePending = new Set();
let cadenceStorageReady = false;
let cadenceStorageOpenPromise = null;
let cadenceStorageLastError = null;

function cadenceStorageError(error, fallback) {
  if (!error) {
    return fallback;
  }
  if (typeof error === "string") {
    return error;
  }
  return String(error.message || error);
}

function cadenceStorageTrack(promise) {
  cadenceStoragePending.add(promise);
  promise
    .catch((error) => {
      cadenceStorageLastError = cadenceStorageError(error, "IndexedDB write failed.");
    })
    .finally(() => {
      cadenceStoragePending.delete(promise);
    });
  return promise;
}

function cadenceStorageTxDone(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(tx.error || new Error("IndexedDB transaction aborted."));
    tx.onerror = () => reject(tx.error || new Error("IndexedDB transaction failed."));
  });
}

function cadenceOpenStorageDb() {
  if (cadenceStorageOpenPromise) {
    return cadenceStorageOpenPromise;
  }

  cadenceStorageOpenPromise = new Promise((resolve, reject) => {
    if (typeof indexedDB === "undefined") {
      reject(new Error("IndexedDB unavailable in this browser."));
      return;
    }

    const request = indexedDB.open("cadence.browser.storage", 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("kv")) {
        db.createObjectStore("kv");
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("Failed to open IndexedDB."));
  });

  return cadenceStorageOpenPromise;
}

async function cadenceReadAllStorageEntries() {
  const db = await cadenceOpenStorageDb();
  const tx = db.transaction("kv", "readonly");
  const store = tx.objectStore("kv");
  const entries = await new Promise((resolve, reject) => {
    const pairs = [];
    const request = store.openCursor();
    request.onsuccess = (event) => {
      const cursor = event.target.result;
      if (!cursor) {
        resolve(pairs);
        return;
      }
      pairs.push([String(cursor.key), cursor.value == null ? null : String(cursor.value)]);
      cursor.continue();
    };
    request.onerror = () => reject(request.error || tx.error || new Error("Failed to read IndexedDB."));
  });
  await cadenceStorageTxDone(tx);
  return entries;
}

function cadenceSlug(value) {
  const trimmed = String(value ?? "").trim();
  const fallback = trimmed.length ? trimmed : "project";
  return fallback
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    || "project";
}

export async function cadenceStoragePrepare() {
  if (cadenceStorageReady) {
    return true;
  }

  const entries = await cadenceReadAllStorageEntries();
  cadenceStorageCache.clear();
  for (const [key, value] of entries) {
    if (typeof key === "string" && typeof value === "string") {
      cadenceStorageCache.set(key, value);
    }
  }
  cadenceStorageReady = true;
  cadenceStorageLastError = null;
  return true;
}

export async function cadenceStorageFlush() {
  await Promise.all(Array.from(cadenceStoragePending));
  if (cadenceStorageLastError) {
    throw new Error(cadenceStorageLastError);
  }
  cadenceStorageLastError = null;
  return true;
}

export function cadenceStorageGet(key) {
  const value = cadenceStorageCache.get(String(key));
  return typeof value === "string" ? value : null;
}

export function cadenceStorageSet(key, value) {
  const normalizedKey = String(key);
  const normalizedValue = String(value);
  cadenceStorageLastError = null;
  cadenceStorageCache.set(normalizedKey, normalizedValue);
  cadenceStorageTrack((async () => {
    const db = await cadenceOpenStorageDb();
    const tx = db.transaction("kv", "readwrite");
    tx.objectStore("kv").put(normalizedValue, normalizedKey);
    await cadenceStorageTxDone(tx);
  })());
}

export function cadenceStorageDelete(key) {
  const normalizedKey = String(key);
  cadenceStorageLastError = null;
  cadenceStorageCache.delete(normalizedKey);
  cadenceStorageTrack((async () => {
    const db = await cadenceOpenStorageDb();
    const tx = db.transaction("kv", "readwrite");
    tx.objectStore("kv").delete(normalizedKey);
    await cadenceStorageTxDone(tx);
  })());
}

export function cadenceWorkspaceUri(name) {
  return `browser://workspace/${cadenceSlug(name)}.cadence.json`;
}

export async function cadencePickOpenProject() {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".json,application/json";

  return await new Promise((resolve, reject) => {
    input.onchange = async () => {
      const file = input.files && input.files[0];
      if (!file) {
        resolve(null);
        return;
      }
      try {
        const text = await file.text();
        const token = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
        cadenceImports.set(token, text);
        resolve(`import://${token}/${file.name || "project.cadence.json"}`);
      } catch (error) {
        reject(error);
      }
    };
    input.click();
  });
}

export function cadenceTakeImportPayload(token) {
  const payload = cadenceImports.get(token) ?? null;
  cadenceImports.delete(token);
  return payload;
}

export function cadenceDownloadText(fileName, text, mimeType) {
  const blob = new Blob([text], { type: mimeType || "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = cadenceStoragePrepare)]
    async fn prepare_storage_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = cadenceStorageFlush)]
    async fn flush_storage_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = cadenceStorageGet)]
    fn storage_get(key: &str) -> Option<String>;

    #[wasm_bindgen(js_name = cadenceStorageSet)]
    fn storage_set(key: &str, value: &str);

    #[wasm_bindgen(js_name = cadenceStorageDelete)]
    fn storage_delete(key: &str);

    #[wasm_bindgen(js_name = cadenceWorkspaceUri)]
    fn workspace_uri(name: &str) -> String;

    #[wasm_bindgen(catch, js_name = cadencePickOpenProject)]
    async fn pick_open_project_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = cadenceTakeImportPayload)]
    fn take_import_payload(token: &str) -> Option<String>;

    #[wasm_bindgen(js_name = cadenceDownloadText)]
    fn download_text(file_name: &str, text: &str, mime_type: &str);
}

pub(crate) async fn prepare_storage() -> Result<(), String> {
    prepare_storage_js().await.map(|_| ()).map_err(js_error)
}

pub(crate) async fn flush_storage() -> Result<(), String> {
    flush_storage_js().await.map(|_| ()).map_err(js_error)
}

pub(crate) fn workspace_project_location(project_name: &str) -> Option<String> {
    let seed = file_name(project_name).unwrap_or_else(|| project_name.trim().to_string());
    Some(workspace_uri(
        canonical_project_name_seed(seed.as_str()).as_str(),
    ))
}

pub(crate) fn export_download_path(file_name: &str) -> String {
    format!("{DOWNLOAD_PREFIX}{}", file_name.trim())
}

pub(crate) fn parent_location(_location: &str) -> Option<String> {
    None
}

pub(crate) fn file_name(location: &str) -> Option<String> {
    location.rsplit('/').next().map(ToOwned::to_owned)
}

pub(crate) fn current_workspace_project() -> Result<Option<WorkspaceProjectSnapshot>, String> {
    let payload = storage_get(WORKSPACE_PAYLOAD_KEY);
    let location = storage_get(WORKSPACE_LOCATION_KEY);
    Ok(payload
        .filter(|payload| !payload.trim().is_empty())
        .map(|payload| WorkspaceProjectSnapshot { location, payload }))
}

pub(crate) fn write_workspace_project(snapshot: &WorkspaceProjectSnapshot) -> Result<(), String> {
    if snapshot.payload.trim().is_empty() {
        return Err("cannot persist empty workspace payload".to_string());
    }

    storage_set(WORKSPACE_PAYLOAD_KEY, snapshot.payload.as_str());
    if let Some(location) = snapshot.location.as_deref() {
        storage_set(WORKSPACE_LOCATION_KEY, location);
    } else {
        storage_delete(WORKSPACE_LOCATION_KEY);
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn clear_workspace_project() -> Result<(), String> {
    storage_delete(WORKSPACE_LOCATION_KEY);
    storage_delete(WORKSPACE_PAYLOAD_KEY);
    Ok(())
}

pub(crate) fn load_project(location: &str) -> Result<String, String> {
    if location.starts_with(WORKSPACE_PREFIX) {
        let Some(snapshot) = current_workspace_project()? else {
            return Err(format!("project not found: {location}"));
        };
        return Ok(snapshot.payload);
    }

    if let Some(remainder) = location.strip_prefix(IMPORT_PREFIX) {
        let token = remainder
            .split('/')
            .next()
            .ok_or_else(|| format!("invalid import uri: {location}"))?;
        return take_import_payload(token)
            .ok_or_else(|| format!("import payload not found: {location}"));
    }

    Err(format!("unsupported web project location: {location}"))
}

pub(crate) fn save_project(location: &str, payload: &str) -> Result<String, String> {
    if payload.trim().is_empty() {
        return Err("cannot save empty payload".to_string());
    }

    let resolved = if location.starts_with(WORKSPACE_PREFIX) {
        location.to_string()
    } else if location.starts_with(DOWNLOAD_PREFIX) {
        let file_name = file_name(location).unwrap_or_else(|| "project.cadence.json".to_string());
        download_text(
            file_name.as_str(),
            payload,
            "application/json;charset=utf-8",
        );
        workspace_project_location(file_name.as_str())
            .expect("web workspace save path should always resolve")
    } else if location.starts_with(IMPORT_PREFIX) {
        workspace_project_location(file_name(location).as_deref().unwrap_or("project"))
            .expect("web import save path should always resolve")
    } else {
        workspace_project_location(location).expect("web save path should always resolve")
    };

    Ok(resolved)
}

pub(crate) async fn pick_open_path(
    _initial_directory: Option<&str>,
) -> Result<Option<String>, String> {
    let value = pick_open_project_js().await.map_err(js_error)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    value
        .as_string()
        .map(Some)
        .ok_or_else(|| "file picker did not return a path".to_string())
}

pub(crate) async fn pick_save_path(
    _initial_directory: Option<&str>,
    initial_name: Option<&str>,
    default_name: &str,
) -> Result<Option<String>, String> {
    let name = initial_name.unwrap_or(default_name).trim();
    let name = if name.is_empty() { default_name } else { name };
    Ok(Some(export_download_path(name)))
}

pub(crate) async fn pick_export_path(
    _initial_directory: Option<&str>,
    file_name: &str,
) -> Result<Option<String>, String> {
    Ok(Some(export_download_path(file_name)))
}

pub(crate) fn save_text_export(location: &str, payload: &str) -> Result<String, String> {
    let file_name = file_name(location).unwrap_or_else(|| "project.cadence.txt".to_string());
    download_text(file_name.as_str(), payload, "text/plain;charset=utf-8");
    Ok(export_download_path(file_name.as_str()))
}

pub(crate) fn recovery_snapshot_location() -> String {
    "browser://recovery/project-recovery.cadence.json".to_string()
}

pub(crate) fn recovery_snapshot_status() -> Result<Option<String>, String> {
    Ok(storage_get(RECOVERY_PAYLOAD_KEY)
        .and_then(|payload| (!payload.trim().is_empty()).then(recovery_snapshot_location)))
}

pub(crate) fn clear_recovery_snapshot() -> Result<String, String> {
    storage_delete(RECOVERY_PAYLOAD_KEY);
    Ok(format!(
        "cleared recovery snapshot at {}",
        recovery_snapshot_location()
    ))
}

pub(crate) fn write_recovery_snapshot(payload: &str) -> Result<String, String> {
    storage_set(RECOVERY_PAYLOAD_KEY, payload);
    Ok(recovery_snapshot_location())
}

pub(crate) fn load_recovery_snapshot() -> Result<String, String> {
    storage_get(RECOVERY_PAYLOAD_KEY).ok_or_else(|| "recovery snapshot not found".to_string())
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "browser storage bridge failed".to_string())
}
