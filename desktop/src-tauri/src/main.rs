#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod notification_helper;
mod notifications;
mod tray;
mod usage_refresh;
mod window;
use anyhow::Context;
use codebeat_lib::*;
use commands::*;
use serde_json::json;
use tauri::{Emitter, Manager};

fn emit(app: &tauri::AppHandle, t: &Task) {
    let _=app.emit("task-updated",json!({"id":t.id,"version":t.updated_at.timestamp_millis(),"serverTimestamp":chrono::Utc::now(),"task":t}));
}
fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        window::shown(app);
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
fn main() {
    if std::env::args().any(|arg| arg == "--notification-helper") {
        if let Err(error) = notification_helper::run() {
            eprintln!("Notification helper: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|arg| arg == "--open-tray") {
                if let Ok(position) = app.cursor_position() {
                    let _ = tray::open(app, position);
                }
                return;
            }
            show(app);
            if let Some(target) = notification_target(&args) {
                let _ = app.emit("notification-open", target);
            }
        }))
        .setup(|app| {
            let dir = codebeat_lib::db::data_dir()?;
            std::fs::create_dir_all(&dir)?;
            codebeat_lib::platform::restrict_data_directory(&dir)?;
            let database = db()?;
            database.recover_stale()?;
            let settings = database.settings()?;
            if let Err(error) = codebeat_lib::platform::runner_path().and_then(|runner| {
                codebeat_lib::platform::configure_pretrigger(
                    &runner,
                    settings.pretrigger_enabled,
                    settings.pretrigger.wake,
                )?;
                codebeat_lib::platform::sync_optimizer_wake(&database, &runner)
            }) {
                eprintln!("CODING_ALLOCATION_SETUP: {error}");
            }
            window::initialize(app.handle())?;
            tauri::tray::TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().context("icon missing")?.clone())
                .tooltip("CodeBeat")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Enter { position, .. }
                    | tauri::tray::TrayIconEvent::Move { position, .. } = &event
                    {
                        notifications::hover(tray.app_handle(), *position);
                    }

                    if let tauri::tray::TrayIconEvent::Click {
                        button,
                        button_state,
                        position,
                        ..
                    } = event
                    {
                        match tray::click_action(button, button_state) {
                            Some(tray::ClickAction::Main) => {
                                if let Err(error) = tray::tray_action(
                                    "open".into(),
                                    None,
                                    tray.app_handle().clone(),
                                ) {
                                    eprintln!("Open main: {error}");
                                    show(tray.app_handle());
                                }
                            }
                            Some(tray::ClickAction::Menu) => {
                                if let Err(error) = tray::open(tray.app_handle(), position) {
                                    eprintln!("Tray panel: {error}");
                                    show(tray.app_handle());
                                }
                            }
                            None => {}
                        }
                    }
                })
                .build(app)?;
            notifications::start(app.handle().clone());
            usage_refresh::start(app.handle())?;
            if get_launch_target().is_none()
                && (std::env::args().any(|a| a == "--background")
                    || db()?.settings()?.startup_minimized
                    || std::env::args().any(|arg| arg == "--open-tray"))
            {
                if let Some(w) = app.get_webview_window("main") {
                    w.hide()?;
                }
            }
            if get_launch_target().is_some()
                || (!std::env::args().any(|arg| arg == "--background" || arg == "--open-tray")
                    && !db()?.settings()?.startup_minimized)
            {
                if let Some(w) = app.get_webview_window("main") {
                    w.show()?;
                    window::shown(app.handle());
                }
            }
            if std::env::args().any(|arg| arg == "--open-tray") {
                tray::open(app.handle(), app.cursor_position()?)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "tray" {
                match event {
                    tauri::WindowEvent::Focused(false) if !notifications::hover_active() => {
                        let _ = window.hide();
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
                }
                return;
            }
            if matches!(
                event,
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                window::monitor_changed(window.app_handle());
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close = db()
                    .and_then(|d| d.settings())
                    .map(|s| s.close_to_tray)
                    .unwrap_or(true);
                api.prevent_close();
                if close {
                    let _ = window.hide();
                } else {
                    let _ = window.emit("exit-requested", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            tray::get_tray_snapshot,
            tray::tray_action,
            window::get_display_layout,
            window::set_display_scale,
            window::save_window_layout,
            notifications::can_present_notices,
            notifications::get_pending_notices,
            notifications::publish_notice,
            notifications::acknowledge_notices,
            notifications::ignore_all_notices,
            notifications::open_notice,
            commands::get_launch_target,
            commands::get_dashboard,
            commands::get_monitoring,
            commands::get_token_usage,
            commands::get_default_settings,
            commands::open_logs_folder,
            commands::open_provider,
            commands::get_conversation_detail,
            commands::list_providers,
            commands::detect_provider,
            commands::refresh_usage,
            usage_refresh::get_usage_refresh,
            commands::consume_reset_credit,
            commands::list_conversations,
            commands::get_session_lock_states,
            commands::create_task,
            commands::update_task,
            commands::cancel_task,
            commands::pause_task,
            commands::run_task_now,
            commands::list_tasks,
            commands::list_run_history,
            commands::get_run_log,
            commands::get_settings,
            commands::preview_pretrigger,
            commands::get_pretrigger_runs,
            commands::get_optimizer_status,
            commands::get_codex_compatibility,
            commands::update_settings,
            commands::get_wake_diagnostics,
            commands::install_maintenance,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("codebeat desktop startup failed");
}
