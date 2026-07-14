use super::model::{PlaylistFolder, PlaylistNode};

pub fn build_json(root_folders: &[PlaylistFolder], now: i64) -> String {
    let folders: Vec<String> = root_folders
        .iter()
        .map(|f| folder_to_json(f, now))
        .collect();
    format!("[{}]", folders.join(","))
}

fn folder_to_json(folder: &PlaylistFolder, now: i64) -> String {
    let children: Vec<String> = folder
        .children
        .iter()
        .map(|n| node_to_json(n, now))
        .collect();
    let child = format!("[{}]", children.join(","));
    let mut fields = vec![
        format!("\"uuid\":{}", json_string(&folder.uuid)),
        format!("\"title\":{}", json_string(&folder.title)),
    ];
    if folder.is_favorite {
        fields.push("\"favorite\":1".to_string());
    }
    fields.push(format!("\"child\":{child}"));
    format!("{{{}}}", fields.join(","))
}

fn node_to_json(node: &PlaylistNode, now: i64) -> String {
    match node {
        PlaylistNode::Folder(folder) => folder_to_json(folder, now),
        PlaylistNode::Sound { uuid, title } => format!(
            "{{\"uuid\":{},\"title\":{},\"add_time\":{now},\"limit_time\":0}}",
            json_string(uuid),
            json_string(title)
        ),
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for c in value.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const NOW: i64 = 1_700_000_000;

    fn decode(json: &str) -> Vec<serde_json::Map<String, Value>> {
        let value: Value = serde_json::from_str(json).expect("JSON invalide");
        value
            .as_array()
            .expect("racine attendue : tableau")
            .iter()
            .map(|v| v.as_object().expect("élément attendu : objet").clone())
            .collect()
    }

    #[test]
    fn folder_with_sound_produces_expected_keys_and_values() {
        let mut folder = PlaylistFolder::new("folder-uuid", "Mon Dossier");
        folder.add_sound("sound-uuid", "Mon Episode");

        let root = decode(&build_json(&[folder], NOW));

        assert_eq!(root.len(), 1);
        assert_eq!(root[0]["uuid"], "folder-uuid");
        assert_eq!(root[0]["title"], "Mon Dossier");
        assert!(
            !root[0].contains_key("favorite"),
            "favorite ne doit apparaître que si is_favorite est vrai"
        );

        let children = root[0]["child"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["uuid"], "sound-uuid");
        assert_eq!(children[0]["title"], "Mon Episode");
        assert_eq!(children[0]["limit_time"], 0);
        assert_eq!(children[0]["add_time"], NOW);
    }

    #[test]
    fn favorite_key_only_present_when_set() {
        let mut favorite = PlaylistFolder::new("fav", "Favoris");
        favorite.is_favorite = true;
        let normal = PlaylistFolder::new("normal", "Normal");

        let root = decode(&build_json(&[favorite, normal], NOW));

        let fav = root.iter().find(|f| f["uuid"] == "fav").unwrap();
        assert_eq!(fav["favorite"], 1);
        let normal = root.iter().find(|f| f["uuid"] == "normal").unwrap();
        assert!(!normal.contains_key("favorite"));
    }

    #[test]
    fn nested_folders_serialize_recursively_at_any_depth() {
        let mut category = PlaylistFolder::new("cat", "Categorie");
        category
            .get_or_create_subfolder("podcast", "Podcast")
            .add_sound("ep", "Episode");

        let root = decode(&build_json(&[category], NOW));

        let podcast_children = root[0]["child"].as_array().unwrap();
        assert_eq!(podcast_children[0]["title"], "Podcast");
        let episode_children = podcast_children[0]["child"].as_array().unwrap();
        assert_eq!(episode_children[0]["title"], "Episode");
    }

    #[test]
    fn special_characters_in_title_are_escaped_into_valid_json() {
        let folder = PlaylistFolder::new("uuid", "Guillemet \" Backslash \\ Tab\tRetour\n");

        let root = decode(&build_json(&[folder], NOW));
        assert_eq!(root[0]["title"], "Guillemet \" Backslash \\ Tab\tRetour\n");
    }

    #[test]
    fn accented_title_round_trips_as_utf8() {
        let folder = PlaylistFolder::new("uuid", "Mystère à l'école");
        let root = decode(&build_json(&[folder], NOW));
        assert_eq!(root[0]["title"], "Mystère à l'école");
    }

    #[test]
    fn empty_root_folders_produces_empty_json_array() {
        assert_eq!(build_json(&[], NOW), "[]");
    }

    #[test]
    fn exact_output_string_locks_key_order() {
        let mut category = PlaylistFolder::new("cat-1", "Histoires");
        category.add_sound("snd-1", "Episode 1");
        let mut favorite = PlaylistFolder::new("fav-1", "Favoris");
        favorite.is_favorite = true;

        let json = build_json(&[category, favorite], NOW);

        let expected = concat!(
            "[{\"uuid\":\"cat-1\",\"title\":\"Histoires\",\"child\":",
            "[{\"uuid\":\"snd-1\",\"title\":\"Episode 1\",\"add_time\":1700000000,\"limit_time\":0}]},",
            "{\"uuid\":\"fav-1\",\"title\":\"Favoris\",\"favorite\":1,\"child\":[]}]"
        );
        assert_eq!(json, expected);
    }
}
