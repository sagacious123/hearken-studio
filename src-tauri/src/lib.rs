mod asr;
mod audio;
mod commands;
mod follow;
mod history;
mod pipeline;
mod presentation;
mod scripture;
mod share;
mod songs;
mod state;

use tauri::{Manager, WindowEvent};

/// Event names shared with the frontend (`src/lib/ipc.ts`).
pub mod events {
    pub const TRANSCRIPT: &str = "pipeline://transcript";
    pub const DETECTION: &str = "pipeline://detection";
    pub const STATUS: &str = "pipeline://status";
    pub const ERROR: &str = "pipeline://error";
    pub const OUTPUT_STATE: &str = "output://state";
    pub const HARDWARE_PROGRESS: &str = "hardware://progress";
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = state::AppState::initialize(app.handle())?;
            app.manage(state);
            let handle = app.handle().clone();
            if let Err(e) = commands::apply_sharing(&handle, &handle.state::<state::AppState>()) {
                log::warn!("local sharing not started: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the operator window takes the projector window and audio threads down with it.
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { .. } = event {
                    if let Some(state) = window.try_state::<state::AppState>() {
                        if let Some(pipeline) = state.pipeline.lock().take() {
                            pipeline.stop();
                        }
                        state.share.stop();
                    }
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_hardware_profile,
            commands::run_hardware_benchmark,
            commands::list_audio_devices,
            commands::start_pipeline,
            commands::stop_pipeline,
            commands::list_translations,
            commands::search_scripture,
            commands::build_slide,
            commands::get_output_state,
            commands::present,
            commands::clear_output,
            commands::set_blackout,
            commands::set_theme,
            commands::step_verse,
            commands::list_monitors,
            commands::open_output_window,
            commands::close_output_window,
            commands::get_share_info,
            commands::show_qr,
            commands::get_plan,
            commands::set_plan,
            commands::list_songs,
            commands::get_song_text,
            commands::save_song,
            commands::delete_song,
            commands::song_slide,
            commands::list_sessions,
            commands::search_archive,
            commands::get_insights,
            commands::export_recap,
            commands::export_ccli_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hearken Studio");
}
