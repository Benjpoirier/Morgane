use serde_json::json;

use merlin_application::sync_episodes_use_case::SyncProgressPhase;
use merlin_domain::library::subscription::Subscription;
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_domain::playlist::tree_edit::TreeEdit;
use merlin_domain::sync::integrity_checker::{Issue, IssueKind, MissingFile};

fn to_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("sérialisation")
}

#[test]
fn subscription_is_camel_case() {
    let mut sub = Subscription::new("https://a.com/feed");
    sub.selected_episode_guids = vec!["g1".into()];
    sub.feed_image_url = Some("https://a.com/i.jpg".into());
    let v = to_value(&sub);
    assert_eq!(v["feedUrl"], "https://a.com/feed");
    assert_eq!(v["selectedEpisodeGuids"], json!(["g1"]));
    assert_eq!(v["feedImageUrl"], "https://a.com/i.jpg");
    assert!(v.get("feed_url").is_none(), "pas de snake_case");
}

#[test]
fn playlist_node_is_internally_tagged_by_kind() {
    let mut folder = PlaylistFolder::new("cat-1", "Histoires");
    folder.is_favorite = true;
    folder.add_sound("snd-1", "Episode");

    let node = PlaylistNode::Folder(folder);
    let v = to_value(&node);
    assert_eq!(v["kind"], "folder");
    assert_eq!(v["uuid"], "cat-1");
    assert_eq!(v["isFavorite"], true);

    assert_eq!(v["children"][0]["kind"], "sound");
    assert_eq!(v["children"][0]["uuid"], "snd-1");

    let sound = PlaylistNode::Sound {
        uuid: "s".into(),
        title: "T".into(),
    };
    assert_eq!(
        to_value(&sound),
        json!({"kind": "sound", "uuid": "s", "title": "T"})
    );
}

#[test]
fn missing_file_and_issue_are_tagged_by_type() {
    assert_eq!(
        to_value(&MissingFile::Image {
            remote_name: "u.jpg".into()
        }),
        json!({"type": "image", "remoteName": "u.jpg"}),
    );
    assert_eq!(
        to_value(&MissingFile::Audio {
            base_uuid: "u".into()
        }),
        json!({"type": "audio", "baseUuid": "u"}),
    );

    let issue = Issue {
        uuid: "u".into(),
        title: "T".into(),
        kind: IssueKind::Folder,
        missing_files: vec![MissingFile::Image {
            remote_name: "u.jpg".into(),
        }],
    };
    let v = to_value(&issue);
    assert_eq!(v["kind"], "folder");
    assert_eq!(v["missingFiles"][0]["type"], "image");
}

#[test]
fn tree_edit_is_tagged_by_type() {
    assert_eq!(
        to_value(&TreeEdit::RenamedFolder {
            uuid: "u".into(),
            new_title: "N".into()
        }),
        json!({"type": "renamedFolder", "uuid": "u", "newTitle": "N"}),
    );
    assert_eq!(
        to_value(&TreeEdit::Moved {
            uuid: "u".into(),
            to_parent_uuid: "p".into()
        }),
        json!({"type": "moved", "uuid": "u", "toParentUuid": "p"}),
    );
}

#[test]
fn sync_progress_phase_is_adjacently_tagged() {
    assert_eq!(
        to_value(&SyncProgressPhase::Connecting),
        json!({"type": "connecting"})
    );
    assert_eq!(
        to_value(&SyncProgressPhase::Sending {
            bytes_done: 10,
            bytes_total: 20
        }),
        json!({"type": "sending", "data": {"bytesDone": 10, "bytesTotal": 20}}),
    );
    assert_eq!(
        to_value(&SyncProgressPhase::Finished { count: 3 }),
        json!({"type": "finished", "data": {"count": 3}}),
    );
    assert_eq!(
        to_value(&SyncProgressPhase::Failed("oops".into())),
        json!({"type": "failed", "data": "oops"}),
    );
}
