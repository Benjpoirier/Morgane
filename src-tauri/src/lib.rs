mod commands;
mod dto;
mod events;
mod state;
mod tree_session;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::connection::test_connection,
            commands::connection::check_internet,
            commands::devices::list_registered_devices,
            commands::devices::set_active_device,
            commands::devices::rename_registered_device,
            commands::devices::remove_registered_device,
            commands::subscriptions::list_subscriptions,
            commands::subscriptions::add_rss,
            commands::subscriptions::add_direct,
            commands::subscriptions::delete_subscription,
            commands::subscriptions::set_selected_guids,
            commands::subscriptions::load_feed,
            commands::subscriptions::search_podcasts,
            commands::subscriptions::curated_podcasts,
            commands::subscriptions::popular_kids_podcasts,
            commands::subscriptions::new_episodes,
            commands::subscriptions::mark_feed_seen,
            commands::sync_state::get_sync_state,
            commands::sync_state::mark_pending_deletion,
            commands::sync_state::set_episode_title_override,
            commands::sync_state::set_episode_number_override,
            commands::sync_state::set_episode_image_override,
            commands::sync_state::set_group_title_override,
            commands::sync_state::remove_category_assignment,
            commands::sync_state::set_category_assignment,
            commands::sync_state::set_folder_image_override,
            commands::pending::guess_numbers,
            commands::pending::episode_uuids,
            commands::pending::compute_pending_groups,
            commands::tree::refresh_tree,
            commands::tree::rename_folder,
            commands::tree::rename_sound,
            commands::tree::rename_pending_group_preview,
            commands::tree::move_node,
            commands::tree::delete_folder,
            commands::tree::cancel_tree_edit,
            commands::tree::add_manual_category,
            commands::tree::remove_manual_category,
            commands::tree::toggle_orphan,
            commands::tree::toggle_all_orphans,
            commands::tree::clear_pending_edits,
            commands::tree::clear_pending_orphan_deletions,
            commands::tree::search_orphans,
            commands::tree::download_thumbnails,
            commands::integrity::check_integrity,
            commands::integrity::repair_integrity,
            commands::sync::start_sync,
            commands::sync::cancel_sync,
            commands::sync::prepare_selection,
            commands::sync::prepared_guids,
            commands::setup::ffmpeg_ready,
            commands::setup::download_ffmpeg,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
