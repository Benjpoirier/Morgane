use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_domain::playlist::{bin_encoder, bin_parser};

fn reference_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../merlin-mock-device/assets/reference_playlist.bin"
    );
    std::fs::read(path).expect("reference_playlist.bin manquant")
}

fn count_nodes(folders: &[PlaylistFolder]) -> (usize, usize) {
    let mut folder_count = 0;
    let mut sound_count = 0;
    fn visit(folder: &PlaylistFolder, folders: &mut usize, sounds: &mut usize) {
        *folders += 1;
        for child in &folder.children {
            match child {
                PlaylistNode::Folder(sub) => visit(sub, folders, sounds),
                PlaylistNode::Sound { .. } => *sounds += 1,
            }
        }
    }
    for folder in folders {
        visit(folder, &mut folder_count, &mut sound_count);
    }
    (folder_count, sound_count)
}

#[test]
fn reference_playlist_bin_parses_to_expected_structure() {
    let data = reference_bytes();
    assert_eq!(
        data.len(),
        19_304,
        "playlist d'exemple : 127 enregistrements x 152 octets"
    );
    assert_eq!(data.len() % bin_parser::ACTUAL_RECORD_SIZE, 0);

    let tree = bin_parser::parse(&data);

    let titles: Vec<&str> = tree.iter().map(|f| f.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Histoires",
            "Documentaires",
            "Calme",
            "Merlin_favorite",
            "Dernier ajouts"
        ]
    );

    let favorite = tree
        .iter()
        .find(|f| f.is_favorite)
        .expect("le favori doit être reconnu");
    assert_eq!(favorite.title, "Merlin_favorite");
    assert!(
        favorite.children.is_empty(),
        "le favori est vide dans cet exemple"
    );

    let (folders, sounds) = count_nodes(&tree);

    assert_eq!(folders, 16, "5 catégories racines + 11 sous-dossiers");
    assert_eq!(sounds, 110);
    assert_eq!(folders + sounds, 126);
}

#[test]
fn reference_playlist_bin_carries_titles_longer_than_thirty_bytes() {
    let tree = bin_parser::parse(&reference_bytes());

    fn longest(folders: &[PlaylistFolder]) -> usize {
        let mut max = 0;
        for folder in folders {
            max = max.max(folder.title.len());
            for child in &folder.children {
                match child {
                    PlaylistNode::Folder(sub) => max = max.max(longest(std::slice::from_ref(sub))),
                    PlaylistNode::Sound { title, .. } => max = max.max(title.len()),
                }
            }
        }
        max
    }

    let longest = longest(&tree);
    assert!(longest > 30, "titre le plus long : {longest} octets");
    assert!(longest <= PlaylistFolder::MAX_TITLE_UTF8_BYTES);
}

#[test]
fn reference_playlist_bin_round_trips_structurally() {
    let original = bin_parser::parse(&reference_bytes());

    let re_encoded = bin_encoder::encode(&original);
    let re_parsed = bin_parser::parse(&re_encoded);

    assert_eq!(
        re_parsed, original,
        "l'arbre doit survivre à l'aller-retour encode -> parse"
    );
}
