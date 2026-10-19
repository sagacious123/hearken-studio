mod asr;
mod audio;
mod backgrounds;
mod commands;
mod follow;
mod history;
mod licence;
mod recap;
mod voice;
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
    /// Listening continues, but the operator should know (e.g. no audio arriving). "" clears it.
    pub const WARNING: &str = "pipeline://warning";
    pub const OUTPUT_STATE: &str = "output://state";
    pub const HARDWARE_PROGRESS: &str = "hardware://progress";
    pub const MODEL_PROGRESS: &str = "models://progress";
    pub const LIBRARY_PROGRESS: &str = "library://progress";
    pub const VOICE_COMMAND: &str = "pipeline://voice-command";
    /// The room started (true) or stopped (false) singing.
    pub const SINGING: &str = "pipeline://singing";
    pub const MEANING_PROGRESS: &str = "meaning://progress";
    /// The trial or licence changed (after an online check).
    pub const LICENCE: &str = "licence://status";
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
            state::AppState::load_meaning(&handle);
            state::AppState::check_licence(&handle);
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
            commands::pipeline_info,
            commands::licence_status,
            commands::activate_licence,
            commands::check_licence_online,
            commands::sign_in,
            commands::sign_up,
            commands::confirm_sign_up,
            commands::sign_out,
            commands::forgot_password,
            commands::list_translations,
            commands::list_languages,
            commands::language_setup,
            commands::search_scripture,
            commands::build_slide,
            commands::list_books,
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
            commands::list_library,
            commands::list_system_fonts,
            commands::open_screen,
            commands::close_screen,
            commands::list_open_screens,
            commands::meaning_status,
            commands::download_meaning_model,
            commands::remove_meaning_model,
            commands::install_translation,
            commands::remove_translation,
            commands::export_ccli_report,
            commands::list_backgrounds,
            commands::save_background,
            commands::delete_background,
            commands::background_path,
            commands::list_models,
            commands::download_model,
            commands::cancel_model_download,
            commands::delete_model,
            commands::test_model_speed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hearken Studio");
}
