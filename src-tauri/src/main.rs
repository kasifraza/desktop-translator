#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod input;
mod translate;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use translate::Settings;

const MAX_CHARS: usize = 6000;

struct AppState {
    settings: Mutex<Settings>,
    client: reqwest::Client,
    busy: AtomicBool,
}

#[derive(Clone, Serialize)]
struct ResultPayload {
    original: String,
    result: String,
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

/// settings.json never contains secrets; they live in the OS credential store.
fn write_settings(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let path = settings_path(app).ok_or("No config directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut clean = s.clone();
    clean.openai_key.clear();
    clean.opencode_key.clear();
    let json = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn load_settings(app: &AppHandle) -> Settings {
    let mut s: Settings = settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Migrate a plain-text key from older versions into the credential store.
    if !s.openai_key.is_empty() && auth::secret_set("openai", &s.openai_key).is_ok() {
        let _ = write_settings(app, &s);
    }
    if s.openai_key.is_empty() {
        s.openai_key = auth::secret_get("openai");
    }
    if !s.opencode_key.is_empty() && auth::secret_set("opencode", &s.opencode_key).is_ok() {
        let _ = write_settings(app, &s);
    }
    if s.opencode_key.is_empty() {
        s.opencode_key = auth::secret_get("opencode");
    }
    s.openrouter_key = auth::secret_get("openrouter");
    s.github_token = auth::secret_get("github-copilot");
    s
}

fn register_hotkey(app: &AppHandle, hk: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if hk.trim().is_empty() {
        return Ok(());
    }
    let sc: Shortcut = hk
        .trim()
        .parse()
        .map_err(|e| format!("Invalid hotkey '{hk}': {e}"))?;
    gs.register(sc).map_err(|e| format!("Hotkey '{hk}' unavailable: {e}"))
}

fn show_panel_impl(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn emit_state(app: &AppHandle, state: &str) {
    let _ = app.emit("bubble-state", state);
}

/// Bubble click / hotkey: grab text from the focused field, translate, paste back.
fn run_convert(app: AppHandle) {
    if app.state::<AppState>().busy.swap(true, Ordering::SeqCst) {
        return;
    }
    emit_state(&app, "busy");
    tauri::async_runtime::spawn(async move {
        let res = convert_inner(&app).await;
        app.state::<AppState>().busy.store(false, Ordering::SeqCst);
        match res {
            Ok(()) => emit_state(&app, "done"),
            Err(e) => {
                emit_state(&app, &format!("error:{e}"));
                let _ = app.emit("panel-error", e);
                show_panel_impl(&app);
            }
        }
    });
}

async fn convert_inner(app: &AppHandle) -> Result<(), String> {
    let original = tauri::async_runtime::spawn_blocking(input::capture)
        .await
        .map_err(|e| e.to_string())??;
    let text = original.trim();
    if text.is_empty() {
        return Err("No text found. Click inside a text box first.".into());
    }
    if text.chars().count() > MAX_CHARS {
        return Err(format!("Text too long (max {MAX_CHARS} characters)."));
    }

    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let result = translate::translate(&state.client, &settings, text).await?;
    if result.is_empty() {
        return Err("Translator returned empty text.".into());
    }

    let _ = app.emit(
        "result",
        ResultPayload { original: text.to_string(), result: result.clone() },
    );

    if settings.mode == "copy" {
        input::set_clipboard(&result)?;
        show_panel_impl(app);
    } else {
        tauri::async_runtime::spawn_blocking(move || input::paste(&result))
            .await
            .map_err(|e| e.to_string())??;
    }
    Ok(())
}

// ---------- commands ----------

#[tauri::command]
fn convert_active(app: AppHandle) {
    run_convert(app);
}

#[tauri::command]
async fn translate_text(state: State<'_, AppState>, text: String) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }
    if text.chars().count() > MAX_CHARS {
        return Err(format!("Text too long (max {MAX_CHARS} characters)."));
    }
    let settings = state.settings.lock().unwrap().clone();
    translate::translate(&state.client, &settings, text).await
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    input::set_clipboard(&text)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<'_, AppState>, mut settings: Settings) -> Result<(), String> {
    register_hotkey(&app, &settings.hotkey)?;
    auth::secret_set("openai", settings.openai_key.trim())?;
    auth::secret_set("opencode", settings.opencode_key.trim())?;
    write_settings(&app, &settings)?;
    let mut cur = state.settings.lock().unwrap();
    settings.openrouter_key = std::mem::take(&mut cur.openrouter_key); // UI never sees these
    settings.github_token = std::mem::take(&mut cur.github_token);
    *cur = settings;
    Ok(())
}

#[derive(Serialize)]
struct AuthStatus {
    openrouter: bool,
    copilot: bool,
}

#[tauri::command]
fn auth_status(state: State<'_, AppState>) -> AuthStatus {
    let s = state.settings.lock().unwrap();
    AuthStatus { openrouter: !s.openrouter_key.is_empty(), copilot: !s.github_token.is_empty() }
}

/// Opens the browser for OpenRouter sign-in, waits for the redirect, stores the key.
#[tauri::command]
async fn oauth_login(app: AppHandle, state: State<'_, AppState>, provider: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if provider == "copilot" {
        let d = auth::copilot_device_start(&state.client).await?;
        let _ = input::set_clipboard(&d.user_code);
        let _ = app.emit("device-code", d.clone());
        app.opener()
            .open_url(d.verification_uri.clone(), None::<&str>)
            .map_err(|e| format!("Could not open browser: {e}"))?;
        let gh = auth::copilot_device_poll(&state.client, &d).await?;
        auth::copilot_token(&state.client, &gh).await?; // verify the subscription now
        auth::secret_set("github-copilot", &gh)?;
        state.settings.lock().unwrap().github_token = gh;
        show_panel_impl(&app);
        return Ok(());
    }
    if provider != "openrouter" {
        return Err(format!("Unknown provider '{provider}'"));
    }
    let pending = auth::openrouter_begin()?;
    app.opener()
        .open_url(pending.auth_url.clone(), None::<&str>)
        .map_err(|e| format!("Could not open browser: {e}"))?;
    let (pending, code) = tauri::async_runtime::spawn_blocking(move || {
        let code = auth::wait_for_code(&pending);
        (pending, code)
    })
    .await
    .map_err(|e| e.to_string())?;
    let key = auth::openrouter_exchange(&state.client, &pending, &code?).await?;
    auth::secret_set("openrouter", &key)?;
    state.settings.lock().unwrap().openrouter_key = key;
    show_panel_impl(&app); // bring the app back in front of the browser
    Ok(())
}

#[tauri::command]
fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    const ALLOWED: [&str; 3] = ["https://opencode.ai/", "https://github.com/", "https://openrouter.ai/"];
    if !ALLOWED.iter().any(|p| url.starts_with(p)) {
        return Err("Link not allowed".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn oauth_cancel() {
    auth::cancel_logins();
}

#[tauri::command]
fn oauth_logout(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    match provider.as_str() {
        "openrouter" => {
            auth::secret_set("openrouter", "")?;
            state.settings.lock().unwrap().openrouter_key.clear();
        }
        "copilot" => {
            auth::secret_set("github-copilot", "")?;
            state.settings.lock().unwrap().github_token.clear();
        }
        _ => return Err(format!("Unknown provider '{provider}'")),
    }
    Ok(())
}

#[tauri::command]
fn show_panel(app: AppHandle) {
    show_panel_impl(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------- app ----------

fn main() {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(1500))
        .timeout(Duration::from_secs(90))
        .build()
        .expect("http client");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_panel_impl(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        run_convert(app.clone());
                    }
                })
                .build(),
        )
        .manage(AppState {
            settings: Mutex::new(Settings::default()),
            client,
            busy: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            convert_active,
            translate_text,
            copy_text,
            get_settings,
            save_settings,
            auth_status,
            oauth_login,
            oauth_cancel,
            open_link,
            oauth_logout,
            show_panel,
            quit_app
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            let settings = load_settings(&handle);
            if let Err(e) = register_hotkey(&handle, &settings.hotkey) {
                eprintln!("{e}");
            }
            *app.state::<AppState>().settings.lock().unwrap() = settings;

            // Remember which app the user was typing in.
            std::thread::spawn(|| loop {
                input::track_foreground();
                std::thread::sleep(Duration::from_millis(120));
            });

            // Bubble: right edge, vertically centred.
            if let Some(b) = app.get_webview_window("bubble") {
                if let Ok(Some(m)) = b.primary_monitor() {
                    let (sz, pos, sf) = (m.size(), m.position(), m.scale_factor());
                    let x = pos.x + sz.width as i32 - (84.0 * sf) as i32;
                    let y = pos.y + sz.height as i32 / 2;
                    let _ = b.set_position(PhysicalPosition::new(x, y));
                }
                let _ = b.show();
            }

            // Tray
            let toggle = MenuItem::with_id(app, "toggle", "Show / hide bubble", true, None::<&str>)?;
            let panel = MenuItem::with_id(app, "panel", "Open translator && settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &panel, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Translate Bubble")
                .menu(&menu)
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "toggle" => {
                        if let Some(b) = app.get_webview_window("bubble") {
                            if b.is_visible().unwrap_or(false) {
                                let _ = b.hide();
                            } else {
                                let _ = b.show();
                            }
                        }
                    }
                    "panel" => show_panel_impl(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Translate Bubble");
}
