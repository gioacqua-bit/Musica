//! Musica — native core.
//!
//! The interface (library, player, visualizer) lives in `src/` and runs in the system webview.
//! This crate gives it what a browser page cannot do on its own:
//! * a native folder picker,
//! * a recursive scan of the music folder with real tag reading (artist, album artist, album,
//!   title, track/disc numbers, duration, embedded artwork) and folder-image cover detection,
//! * fast transfer of audio and image bytes to the webview,
//! * a decoder fallback that converts any format the webview cannot play into WAV,
//! * storage of the playlists in the app's data folder.
//!
//! Every file request is checked against the currently open music folder.

mod decode;
mod library;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::ipc::Response;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct AppState {
    /// Canonical path of the music folder currently open; file access is limited to it.
    root: Mutex<Option<PathBuf>>,
}

/// Resolves `path` and makes sure it lies inside the open music folder.
fn guarded(state: &State<'_, AppState>, path: &str) -> Result<PathBuf, String> {
    let root = state
        .root
        .lock()
        .map_err(|_| "internal state error".to_string())?
        .clone()
        .ok_or_else(|| "No music folder is open".to_string())?;
    let p = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    if !p.starts_with(&root) {
        return Err("Access outside the music folder was refused".into());
    }
    Ok(p)
}

/// Opens the system folder picker. Returns `None` if the user cancels.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Choose your music folder")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// A folder passed on the command line (`musica /path/to/music`, or "Open with Musica"), if any.
#[tauri::command]
fn initial_folder() -> Option<String> {
    std::env::args()
        .skip(1)
        .map(PathBuf::from)
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Scans `root` recursively and returns every audio file with its tags, plus cover candidates.
#[tauri::command]
async fn scan_library(root: String, state: State<'_, AppState>) -> Result<library::Library, String> {
    let root = PathBuf::from(&root)
        .canonicalize()
        .map_err(|e| format!("{root}: {e}"))?;
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    *state.root.lock().map_err(|_| "internal state error".to_string())? = Some(root.clone());
    tauri::async_runtime::spawn_blocking(move || library::scan(&root))
        .await
        .map_err(|e| e.to_string())
}

/// Returns the raw bytes of an audio or image file inside the music folder.
#[tauri::command]
async fn read_file(path: String, state: State<'_, AppState>) -> Result<Response, String> {
    let p = guarded(&state, &path)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(&p))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// Returns the first picture embedded in an audio file's tags.
#[tauri::command]
async fn embedded_cover(path: String, state: State<'_, AppState>) -> Result<Response, String> {
    let p = guarded(&state, &path)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || library::embedded_picture(&p))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Response::new(bytes))
}

/// Decodes an audio file to 16-bit PCM WAV, for formats the system webview cannot play itself.
#[tauri::command]
async fn decode_wav(path: String, state: State<'_, AppState>) -> Result<Response, String> {
    let p = guarded(&state, &path)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || decode::to_wav(&p))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Response::new(bytes))
}

/// `<app data folder>/playlists.json` (created on first save).
fn playlists_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("playlists.json"))
}

/// Returns the stored playlists as JSON, or an empty string before the first save.
#[tauri::command]
fn load_playlists(app: tauri::AppHandle) -> Result<String, String> {
    match std::fs::read_to_string(playlists_file(&app)?) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Stores the playlists. The JSON is validated and written atomically (temporary file, then
/// rename), so a crash during a save cannot leave a half-written file behind.
#[tauri::command]
fn save_playlists(app: tauri::AppHandle, json: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("invalid playlist data: {e}"))?;
    let path = playlists_file(&app)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            initial_folder,
            scan_library,
            read_file,
            embedded_cover,
            decode_wav,
            load_playlists,
            save_playlists
        ])
        .run(tauri::generate_context!())
        .expect("error while running Musica");
}
