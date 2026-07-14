use tauri::State;

use merlin_domain::library::repositories::SubscriptionRepository;
use merlin_domain::library::subscription::Subscription;
use merlin_domain::podcasts::episode::Podcast;
use merlin_domain::podcasts::search::PodcastSearchResult;
use merlin_infra::persistence::db;
use merlin_infra::persistence::seen_repository::SqliteSeenRepository;
use merlin_infra::persistence::subscription_repository::SqliteSubscriptionRepository;

use crate::state::AppState;

fn repo(state: &AppState) -> Result<SqliteSubscriptionRepository, String> {
    let device_id = state.read_device_id();
    db::open(&state.db_path)
        .map(|connection| SqliteSubscriptionRepository::new(connection, device_id))
        .map_err(|e| format!("base locale inaccessible : {e}"))
}

#[tauri::command]
pub fn list_subscriptions(state: State<AppState>) -> Result<Vec<Subscription>, String> {
    Ok(repo(&state)?.all())
}

#[tauri::command]
pub fn add_rss(url: String, state: State<AppState>) -> Result<(), String> {
    repo(&state)?.add(Subscription::new(&url));
    Ok(())
}

#[tauri::command]
pub fn add_direct(
    title: String,
    audio_url: String,
    image_url: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let subscription = Subscription {
        title: title.clone(),
        kind: "direct".into(),
        direct_audio_url: Some(audio_url),
        direct_title: Some(title),
        direct_image_url: image_url,
        ..Subscription::new(Subscription::new_direct_id())
    };
    repo(&state)?.add(subscription);
    Ok(())
}

#[tauri::command]
pub fn delete_subscription(feed_url: String, state: State<AppState>) -> Result<(), String> {
    repo(&state)?.delete(&feed_url);
    Ok(())
}

#[tauri::command]
pub fn set_selected_guids(
    feed_url: String,
    guids: Vec<String>,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.update_selected_episode_guids(&feed_url, &guids);
    Ok(())
}

#[tauri::command]
pub async fn load_feed(feed_url: String, state: State<'_, AppState>) -> Result<Podcast, String> {
    let podcast = merlin_infra::podcasts::rss_fetcher::fetch(&feed_url)
        .await
        .map_err(|e| e.to_string())?;

    let mut repository = repo(&state)?;
    let title_is_empty = repository
        .all()
        .iter()
        .find(|s| s.feed_url == feed_url)
        .map(|s| s.title.is_empty());
    if let Some(title_is_empty) = title_is_empty {
        let new_title = title_is_empty.then(|| podcast.title.clone());
        let new_image = podcast.image_url.clone();
        if new_title.is_some() || new_image.is_some() {
            repository.update_feed_metadata(&feed_url, new_title.as_deref(), new_image.as_deref());
        }
    }
    Ok(podcast)
}

#[tauri::command]
pub async fn search_podcasts(query: String) -> Result<Vec<PodcastSearchResult>, String> {
    merlin_infra::podcasts::podcast_search::search(&query).await
}

#[tauri::command]
pub fn curated_podcasts() -> Vec<PodcastSearchResult> {
    merlin_domain::podcasts::curated::list()
}

#[tauri::command]
pub async fn popular_kids_podcasts() -> Result<Vec<PodcastSearchResult>, String> {
    merlin_infra::podcasts::podcast_search::popular_kids().await
}

#[tauri::command]
pub fn new_episodes(
    feed_url: String,
    guids: Vec<String>,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    let connection =
        db::open(&state.db_path).map_err(|e| format!("base locale inaccessible : {e}"))?;
    Ok(SqliteSeenRepository::new(connection).new_episodes(&feed_url, &guids))
}

#[tauri::command]
pub fn mark_feed_seen(
    feed_url: String,
    guids: Vec<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let connection =
        db::open(&state.db_path).map_err(|e| format!("base locale inaccessible : {e}"))?;
    SqliteSeenRepository::new(connection).mark_seen(&feed_url, &guids);
    Ok(())
}
