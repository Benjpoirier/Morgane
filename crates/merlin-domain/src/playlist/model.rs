#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistFolder {
    pub uuid: String,
    pub title: String,
    pub children: Vec<PlaylistNode>,
    pub is_favorite: bool,

    pub is_synthetic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlaylistNode {
    Folder(PlaylistFolder),
    Sound { uuid: String, title: String },
}

impl PlaylistFolder {
    pub const MAX_TITLE_UTF8_BYTES: usize = 66;

    pub fn new(uuid: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            title: title.into(),
            children: Vec::new(),
            is_favorite: false,
            is_synthetic: false,
        }
    }

    pub fn add_sound(&mut self, uuid: impl Into<String>, title: impl Into<String>) {
        let uuid = uuid.into();
        for child in &self.children {
            if let PlaylistNode::Sound { uuid: existing, .. } = child
                && *existing == uuid
            {
                return;
            }
        }
        self.children.push(PlaylistNode::Sound {
            uuid,
            title: title.into(),
        });
    }

    pub fn get_or_create_subfolder(&mut self, uuid: &str, title: &str) -> &mut PlaylistFolder {
        let position = self
            .children
            .iter()
            .position(|child| matches!(child, PlaylistNode::Folder(f) if f.uuid == uuid));
        let index = match position {
            Some(index) => index,
            None => {
                self.children
                    .push(PlaylistNode::Folder(PlaylistFolder::new(uuid, title)));
                self.children.len() - 1
            }
        };
        match &mut self.children[index] {
            PlaylistNode::Folder(folder) => folder,
            PlaylistNode::Sound { .. } => {
                unreachable!("l'index pointe un dossier par construction")
            }
        }
    }

    pub fn remove_child(&mut self, uuid: &str) -> Option<PlaylistNode> {
        let index = self
            .children
            .iter()
            .position(|child| child.uuid() == uuid)?;
        Some(self.children.remove(index))
    }

    pub fn rename_sound(&mut self, uuid: &str, new_title: &str) {
        for child in &mut self.children {
            if let PlaylistNode::Sound {
                uuid: existing,
                title,
            } = child
                && existing == uuid
            {
                *title = new_title.to_string();
                return;
            }
        }
    }
}

impl PlaylistNode {
    pub fn uuid(&self) -> &str {
        match self {
            PlaylistNode::Folder(folder) => &folder.uuid,
            PlaylistNode::Sound { uuid, .. } => uuid,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PlaylistNode::Folder(folder) => &folder.title,
            PlaylistNode::Sound { title, .. } => title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_or_create_subfolder_returns_same_node_on_second_call() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent.get_or_create_subfolder("child", "Enfant").title = "Modifie".to_string();
        let second = parent.get_or_create_subfolder("child", "Titre Ignore");
        assert_eq!(
            second.title, "Modifie",
            "le second appel doit retrouver le MÊME dossier"
        );
        assert_eq!(parent.children.len(), 1);
    }

    #[test]
    fn remove_child_returns_removed_node_and_deletes_it_from_children() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent.add_sound("ep-1", "Un");
        parent.add_sound("ep-2", "Deux");

        let removed = parent.remove_child("ep-1");

        assert_eq!(
            removed.map(|n| n.uuid().to_string()),
            Some("ep-1".to_string())
        );
        let uuids: Vec<&str> = parent.children.iter().map(|c| c.uuid()).collect();
        assert_eq!(uuids, ["ep-2"]);
    }

    #[test]
    fn remove_child_returns_none_when_uuid_not_found() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        assert!(parent.remove_child("introuvable").is_none());
    }

    #[test]
    fn rename_sound_preserves_position_among_siblings() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent.add_sound("ep-1", "Un");
        parent.add_sound("ep-2", "Deux");
        parent.add_sound("ep-3", "Trois");

        parent.rename_sound("ep-2", "Deux Renomme");

        let titles: Vec<&str> = parent.children.iter().map(|c| c.title()).collect();
        assert_eq!(titles, ["Un", "Deux Renomme", "Trois"]);
        let uuids: Vec<&str> = parent.children.iter().map(|c| c.uuid()).collect();
        assert_eq!(uuids, ["ep-1", "ep-2", "ep-3"]);
    }

    #[test]
    fn rename_sound_is_no_op_when_target_is_a_folder_not_a_sound() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent.get_or_create_subfolder("folder-uuid", "Sous-dossier");
        parent.rename_sound("folder-uuid", "Nouveau Titre");
        let PlaylistNode::Folder(sub) = &parent.children[0] else {
            panic!("attendu un dossier");
        };
        assert_eq!(
            sub.title, "Sous-dossier",
            "rename_sound ne doit affecter que les sons, jamais les dossiers"
        );
    }

    #[test]
    fn add_sound_is_idempotent_per_uuid_within_same_folder() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent.add_sound("ep-1", "Un");
        parent.add_sound("ep-1", "Un (doublon)");
        assert_eq!(
            parent.children.len(),
            1,
            "finding R6 : pas de doublon par uuid dans un même dossier"
        );
    }
}
