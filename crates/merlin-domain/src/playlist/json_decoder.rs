use serde_json::{Map, Value};

use super::model::{PlaylistFolder, PlaylistNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    MalformedJson,

    RootIsNotAnArray,

    InvalidNode,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::MalformedJson => write!(f, "JSON illisible"),
            DecodeError::RootIsNotAnArray => write!(f, "l'element racine n'est pas un tableau"),
            DecodeError::InvalidNode => write!(f, "noeud invalide (uuid/title manquant)"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn decode(data: &[u8]) -> Result<Vec<PlaylistFolder>, DecodeError> {
    let json: Value = serde_json::from_slice(data).map_err(|_| DecodeError::MalformedJson)?;
    let Value::Array(root) = json else {
        return Err(DecodeError::RootIsNotAnArray);
    };
    root.iter()
        .map(|element| {
            let dict = element.as_object().ok_or(DecodeError::RootIsNotAnArray)?;
            decode_folder(dict)
        })
        .collect()
}

fn decode_folder(dict: &Map<String, Value>) -> Result<PlaylistFolder, DecodeError> {
    let (Some(uuid), Some(title)) = (
        dict.get("uuid").and_then(Value::as_str),
        dict.get("title").and_then(Value::as_str),
    ) else {
        return Err(DecodeError::InvalidNode);
    };
    let mut folder = PlaylistFolder::new(uuid, title);
    if dict.get("favorite").and_then(Value::as_i64) == Some(1) {
        folder.is_favorite = true;
    }

    let children: Vec<&Map<String, Value>> = dict
        .get("child")
        .and_then(Value::as_array)
        .and_then(|array| {
            array
                .iter()
                .map(Value::as_object)
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_default();
    folder.children = children
        .into_iter()
        .map(decode_node)
        .collect::<Result<_, _>>()?;
    Ok(folder)
}

fn decode_node(dict: &Map<String, Value>) -> Result<PlaylistNode, DecodeError> {
    if dict.contains_key("child") {
        return Ok(PlaylistNode::Folder(decode_folder(dict)?));
    }
    let (Some(uuid), Some(title)) = (
        dict.get("uuid").and_then(Value::as_str),
        dict.get("title").and_then(Value::as_str),
    ) else {
        return Err(DecodeError::InvalidNode);
    };
    Ok(PlaylistNode::Sound {
        uuid: uuid.to_string(),
        title: title.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::{bin_encoder, bin_parser, builder};
    use super::*;

    fn make_tree() -> Vec<PlaylistFolder> {
        let mut category = PlaylistFolder::new("cat-1", "Histoires");
        let mut group = PlaylistFolder::new("grp-1", "Mystere a l'ecole");
        group.add_sound("snd-1", "Episode 1");
        group.add_sound("snd-2", "Episode 2");
        category.children = vec![PlaylistNode::Folder(group)];

        let mut favorite = PlaylistFolder::new("fav-1", "Favoris");
        favorite.is_favorite = true;

        vec![category, favorite]
    }

    #[test]
    fn decode_round_trips_builder_output() {
        let tree = make_tree();
        let json = builder::build_json(&tree, 1_700_000_000);

        let decoded = decode(json.as_bytes()).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].uuid, "cat-1");
        assert_eq!(decoded[0].title, "Histoires");
        assert_eq!(decoded[0].children.len(), 1);
        let PlaylistNode::Folder(group) = &decoded[0].children[0] else {
            panic!("attendu un dossier");
        };
        assert_eq!(group.uuid, "grp-1");
        let uuids: Vec<&str> = group.children.iter().map(|c| c.uuid()).collect();
        assert_eq!(uuids, ["snd-1", "snd-2"]);

        assert!(decoded[1].is_favorite);
        assert!(!decoded[0].is_favorite);
    }

    #[test]
    fn decode_rejects_non_array_root() {
        assert_eq!(
            decode(b"{\"uuid\":\"x\"}"),
            Err(DecodeError::RootIsNotAnArray)
        );
    }

    #[test]
    fn decode_rejects_malformed_json() {
        assert_eq!(decode(b"not json at all"), Err(DecodeError::MalformedJson));
    }

    #[test]
    fn encode_then_parse_round_trips_structure() {
        let tree = make_tree();

        let binary = bin_encoder::encode(&tree);
        let parsed = bin_parser::parse(&binary);

        assert_eq!(parsed.len(), 2, "attendu 2 dossiers");
        assert_eq!(parsed[0].uuid, "cat-1");
        assert_eq!(parsed[0].children.len(), 1);
        let PlaylistNode::Folder(group) = &parsed[0].children[0] else {
            panic!("attendu un dossier");
        };
        assert_eq!(group.uuid, "grp-1");
        let uuids: Vec<&str> = group.children.iter().map(|c| c.uuid()).collect();
        assert_eq!(uuids, ["snd-1", "snd-2"]);
        let PlaylistNode::Sound { title, .. } = &group.children[0] else {
            panic!("attendu un son");
        };
        assert_eq!(title, "Episode 1");
    }

    #[test]
    fn an_empty_folder_survives_the_round_trip() {
        let empty = PlaylistFolder::new("empty-1", "Vide");
        let binary = bin_encoder::encode(&[empty]);
        let parsed = bin_parser::parse(&binary);

        assert_eq!(parsed.len(), 1, "un dossier vide reste un dossier");
        assert_eq!(parsed[0].uuid, "empty-1");
    }

    #[test]
    fn encode_rewrites_favorite_folder_title_for_binary_recognition() {
        let mut favorite = PlaylistFolder::new("fav-1", "Favoris");
        favorite.is_favorite = true;

        let binary = bin_encoder::encode(&[favorite]);
        let parsed = bin_parser::parse(&binary);

        let parsed_favorite = parsed.iter().find(|f| f.is_favorite);
        let parsed_favorite =
            parsed_favorite.expect("le parseur doit reconnaître le favori après l'aller-retour");
        assert_eq!(parsed_favorite.title, bin_encoder::FAVORITE_RECORD_TITLE);
    }

    #[test]
    fn full_pipeline_tree_to_json_to_bin_to_tree() {
        let tree = make_tree();

        let json = builder::build_json(&tree, 1_700_000_000);
        let decoded = decode(json.as_bytes()).unwrap();
        let binary = bin_encoder::encode(&decoded);
        let parsed = bin_parser::parse(&binary);

        let uuids: Vec<&str> = parsed.iter().map(|f| f.uuid.as_str()).collect();
        assert_eq!(uuids, ["cat-1", "fav-1"]);
        assert!(parsed[1].is_favorite);
    }

    #[test]
    fn encode_empty_tree_produces_no_folders() {
        let binary = bin_encoder::encode(&[]);
        assert!(bin_parser::parse(&binary).is_empty());
    }

    #[test]
    fn encode_truncates_overlong_title_rather_than_overflowing_record() {
        let mut folder = PlaylistFolder::new("cat-1", "x".repeat(200));
        folder.add_sound("placeholder", "placeholder");

        let binary = bin_encoder::encode(&[folder]);
        let parsed = bin_parser::parse(&binary);

        assert_eq!(parsed.len(), 1, "attendu 1 dossier");
        assert!(parsed[0].title.len() <= PlaylistFolder::MAX_TITLE_UTF8_BYTES);
    }
}
