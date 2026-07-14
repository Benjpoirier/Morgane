#[path = "support/fake_sync_state_repository.rs"]
mod fake_sync_state_repository;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;

use fake_sync_state_repository::FakeSyncStateRepository;
use merlin_application::sync_episodes_use_case::{
    RunCallbacks, SyncEpisodesUseCase, SyncProgressPhase,
};
use merlin_domain::library::category_assignment::PodcastCategoryAssignment;
use merlin_domain::library::repositories::{SyncStateRepository, group_override_key};
use merlin_domain::library::subscription::Subscription;
use merlin_domain::playlist::model::PlaylistFolder;
use merlin_domain::playlist::tree_edit::TreeEdit;
use merlin_domain::podcasts::episode::Episode;
use merlin_infra::podcasts::audio_converter;
use merlin_mock_device::FakeMerlinDevice;

fn make_work_dir() -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("SyncEpisodesUseCaseTests-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("workDir");
    dir
}

fn mark_already_prepared(guid: &str, work_dir: &std::path::Path) {
    let (audio, image) = audio_converter::output_paths(guid, work_dir);
    std::fs::create_dir_all(audio.parent().unwrap()).unwrap();
    std::fs::write(&audio, [0x01, 0x02, 0x03]).unwrap();
    std::fs::write(&image, [0xFF, 0xD8, 0xFF]).unwrap();
}

fn make_episode(guid: &str, title: &str) -> Episode {
    Episode {
        guid: guid.to_string(),
        title: title.to_string(),
        audio_url: "https://example.com/a.mp3".to_string(),
        image_url: None,
        published_at: None,
        duration: None,
    }
}

#[derive(Default)]
struct Recorder {
    phases: Mutex<Vec<SyncProgressPhase>>,
    logs: Mutex<Vec<String>>,
    deletions_completed_calls: Mutex<Vec<Vec<String>>>,
    tree_edits_applied_called: Mutex<bool>,
}

impl Recorder {
    fn last_phase(&self) -> Option<SyncProgressPhase> {
        self.phases.lock().unwrap().last().cloned()
    }
}

async fn run(
    use_case: &mut SyncEpisodesUseCase<FakeSyncStateRepository>,
    pairs: &[(Subscription, Episode)],
    port: u16,
    already_synced: HashSet<String>,
    files_to_delete: HashMap<String, Vec<String>>,
    tree_edits: Vec<TreeEdit>,
) -> Recorder {
    let recorder = Recorder::default();
    {
        let on_phase = |phase: SyncProgressPhase| recorder.phases.lock().unwrap().push(phase);
        let on_log = |message: &str| recorder.logs.lock().unwrap().push(message.to_string());
        let on_current_step = |_: Option<&str>, _: f64| {};
        let on_episode_uploaded = |_: &str| {};
        let deletions_completed = |uuids: &[String]| {
            recorder
                .deletions_completed_calls
                .lock()
                .unwrap()
                .push(uuids.to_vec())
        };
        let tree_edits_applied = || *recorder.tree_edits_applied_called.lock().unwrap() = true;
        let callbacks = RunCallbacks {
            on_phase: &on_phase,
            on_log: &on_log,
            on_current_step: &on_current_step,
            on_episode_uploaded: &on_episode_uploaded,
            deletions_completed: &deletions_completed,
            tree_edits_applied: &tree_edits_applied,
        };
        use_case
            .run(
                pairs,
                "127.0.0.1",
                port,
                &already_synced,
                &files_to_delete,
                tree_edits,
                &callbacks,
            )
            .await;
    }
    recorder
}

fn assignments_for(feed_url: &str) -> HashMap<String, PodcastCategoryAssignment> {
    HashMap::from([(
        group_override_key(feed_url, ""),
        PodcastCategoryAssignment {
            feed_url: feed_url.to_string(),
            group_key: String::new(),
            target_category_uuid: "cat-1".to_string(),
            target_category_title: "Histoires".to_string(),
        },
    )])
}

#[tokio::test]
async fn no_episodes_and_no_files_to_delete_fails_immediately() {
    let mut use_case =
        SyncEpisodesUseCase::new(FakeSyncStateRepository::default(), make_work_dir());
    let recorder = run(
        &mut use_case,
        &[],
        35100,
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    let Some(SyncProgressPhase::Failed(message)) = recorder.last_phase() else {
        panic!("attendu Failed, obtenu {:?}", recorder.last_phase());
    };
    assert!(message.contains("aucun changement"));
}

#[tokio::test]
async fn a_lone_image_override_does_not_fail_the_guard() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let mut repo = FakeSyncStateRepository::default();
    repo.set_folder_image_override("cat-1", Some("https://example.com/img.jpg"));
    let mut use_case = SyncEpisodesUseCase::new(repo, make_work_dir());

    let recorder = run(
        &mut use_case,
        &[],
        device.port(),
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    if let Some(SyncProgressPhase::Failed(message)) = recorder.last_phase() {
        assert!(
            !message.contains("aucun changement"),
            "la garde a rejeté un override d'image seul"
        );
    }
}

#[tokio::test]
async fn overlong_title_blocks_before_any_network_access() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let mut use_case =
        SyncEpisodesUseCase::new(FakeSyncStateRepository::default(), make_work_dir());
    let subscription = Subscription {
        title: "Podcast".into(),
        ..Subscription::new("https://a.com")
    };

    let too_long = "x".repeat(PlaylistFolder::MAX_TITLE_UTF8_BYTES + 1);
    let episode = make_episode("guid-1", &too_long);

    let recorder = run(
        &mut use_case,
        &[(subscription, episode)],
        device.port(),
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    let Some(SyncProgressPhase::Failed(message)) = recorder.last_phase() else {
        panic!("attendu Failed, obtenu {:?}", recorder.last_phase());
    };
    assert!(message.contains("trop long"));
    let (files, _) = device.snapshot();
    assert!(
        files.is_empty(),
        "aucun accès réseau ne doit avoir eu lieu avant la validation des titres"
    );
}

#[tokio::test]
async fn new_episode_without_category_assignment_is_skipped_not_synced() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let work_dir = make_work_dir();
    let mut use_case =
        SyncEpisodesUseCase::new(FakeSyncStateRepository::default(), work_dir.clone());
    let subscription = Subscription {
        title: "Podcast".into(),
        ..Subscription::new("https://a.com")
    };
    let episode = make_episode("guid-1", "Episode Un");
    mark_already_prepared(&episode.guid, &work_dir);

    let recorder = run(
        &mut use_case,
        &[(subscription, episode)],
        device.port(),
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    assert!(
        recorder
            .logs
            .lock()
            .unwrap()
            .iter()
            .any(|m| m.contains("pas envoye")),
        "doit journaliser pourquoi l'épisode est exclu"
    );

    let Some(SyncProgressPhase::Failed(message)) = recorder.last_phase() else {
        panic!("attendu Failed, obtenu {:?}", recorder.last_phase());
    };
    assert!(message.contains("aucun episode"));
}

#[tokio::test]
async fn assigned_new_episode_syncs_and_persists_via_repository() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let work_dir = make_work_dir();
    let repo = FakeSyncStateRepository {
        assignments: assignments_for("https://a.com"),
        ..Default::default()
    };
    let mut use_case = SyncEpisodesUseCase::new(repo, work_dir.clone());
    let subscription = Subscription {
        title: "Podcast".into(),
        ..Subscription::new("https://a.com")
    };
    let episode = make_episode("guid-1", "Episode Un");
    mark_already_prepared(&episode.guid, &work_dir);

    let recorder = run(
        &mut use_case,
        &[(subscription, episode.clone())],
        device.port(),
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    let Some(SyncProgressPhase::Finished { count }) = recorder.last_phase() else {
        panic!("attendu Finished, obtenu {:?}", recorder.last_phase());
    };
    assert_eq!(count, 1);
    let uuid = audio_converter::episode_uuid(&episode.guid);
    let synced_uuids: Vec<String> = use_case
        .repository()
        .synced_records()
        .into_iter()
        .map(|r| r.episode_uuid)
        .collect();
    assert_eq!(
        synced_uuids,
        [uuid],
        "doit être persisté via SyncStateRepository après succès"
    );
}

#[tokio::test]
async fn deletions_completed_callback_fires_after_successful_deletion() {
    let initial_files = HashMap::from([("to-delete.mp3".to_string(), vec![0x01u8])]);
    let device = FakeMerlinDevice::start(0, initial_files, false, None)
        .await
        .expect("mock");
    let mut repo = FakeSyncStateRepository::default();
    repo.record_synced("to-delete", "T", "D", Utc::now());
    let mut use_case = SyncEpisodesUseCase::new(repo, make_work_dir());

    let recorder = run(
        &mut use_case,
        &[],
        device.port(),
        HashSet::new(),
        HashMap::from([("to-delete".to_string(), vec!["to-delete.mp3".to_string()])]),
        Vec::new(),
    )
    .await;

    assert_eq!(
        *recorder.deletions_completed_calls.lock().unwrap(),
        vec![vec!["to-delete".to_string()]]
    );
    assert!(
        use_case.repository().synced_records().is_empty(),
        "le SyncedRecord correspondant doit être supprimé via le repository"
    );
}

#[tokio::test]
async fn tree_edits_applied_callback_fires_only_when_tree_edits_provided() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let work_dir = make_work_dir();
    let mut repo = FakeSyncStateRepository::default();
    let subscription = Subscription {
        title: "Podcast".into(),
        ..Subscription::new("https://a.com")
    };
    let episode = make_episode("guid-1", "Episode Un");
    mark_already_prepared(&episode.guid, &work_dir);
    let uuid = audio_converter::episode_uuid(&episode.guid);
    repo.record_synced(&uuid, &episode.title, "Podcast", Utc::now());
    let mut use_case = SyncEpisodesUseCase::new(repo, work_dir);

    let recorder = run(
        &mut use_case,
        &[(subscription, episode)],
        device.port(),
        HashSet::from([uuid]),
        HashMap::new(),
        vec![TreeEdit::RenamedFolder {
            uuid: "whatever".into(),
            new_title: "Nouveau".into(),
        }],
    )
    .await;

    let Some(SyncProgressPhase::Finished { .. }) = recorder.last_phase() else {
        panic!("attendu Finished, obtenu {:?}", recorder.last_phase());
    };
    assert!(*recorder.tree_edits_applied_called.lock().unwrap());
}

fn make_tiny_jpeg_file(work_dir: &std::path::Path) -> PathBuf {
    let path = work_dir.join(format!("fake-feed-image-{}.jpg", uuid::Uuid::new_v4()));
    let img = image::RgbImage::from_pixel(4, 4, image::Rgb([255, 0, 0]));
    image::DynamicImage::ImageRgb8(img)
        .save_with_format(&path, image::ImageFormat::Jpeg)
        .unwrap();
    path
}

#[tokio::test]
async fn episode_without_own_image_falls_back_to_podcast_image() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let work_dir = make_work_dir();

    let episode = make_episode("guid-1", "Episode Un");

    let (audio_path, _) = audio_converter::output_paths(&episode.guid, &work_dir);
    std::fs::create_dir_all(audio_path.parent().unwrap()).unwrap();
    std::fs::write(&audio_path, [0x01]).unwrap();

    let feed_image_path = make_tiny_jpeg_file(&work_dir);

    let repo = FakeSyncStateRepository {
        assignments: assignments_for("https://a.com"),
        ..Default::default()
    };
    let mut use_case = SyncEpisodesUseCase::new(repo, work_dir.clone());
    let subscription = Subscription {
        title: "Podcast".into(),
        feed_image_url: Some(format!("file://{}", feed_image_path.display())),
        ..Subscription::new("https://a.com")
    };

    let recorder = run(
        &mut use_case,
        &[(subscription, episode.clone())],
        device.port(),
        HashSet::new(),
        HashMap::new(),
        Vec::new(),
    )
    .await;

    let Some(SyncProgressPhase::Finished { count }) = recorder.last_phase() else {
        panic!("attendu Finished, obtenu {:?}", recorder.last_phase());
    };
    assert_eq!(count, 1);
    assert!(
        recorder
            .logs
            .lock()
            .unwrap()
            .iter()
            .any(|m| m.contains("reutilisation de l'image du podcast")),
    );

    let (_, episode_image_path) = audio_converter::output_paths(&episode.guid, &work_dir);
    assert!(episode_image_path.exists());
}
