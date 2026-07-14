pub fn update_playlist_status(status: u8) -> String {
    match status {
        0x00 => "succes".into(),
        0x01 => "trame trop courte".into(),
        0x02 => "extension de nom de fichier invalide (doit finir par .json)".into(),
        0x03 => "playlist.json introuvable sur l'enceinte".into(),
        0x04 => "echec de minification du JSON".into(),
        0x05 => "echec d'ouverture ou d'initialisation du fichier JSON".into(),
        0x06 => "impossible d'ouvrir la playlist binaire temporaire".into(),
        0x07 => "l'element racine du JSON n'est pas un tableau".into(),
        0x08 => "taille du tableau racine invalide (au moins une categorie requise)".into(),
        0x0F => "echec du renommage du fichier binaire temporaire vers le fichier final".into(),
        0x10 => "type d'entree invalide dans le fichier binaire genere".into(),
        0x11 => "image (.jpg) introuvable sur la carte SD, pour un dossier/categorie ou une piste/musique".into(),
        0x12 => "fichier audio (.mp3/.aac) d'un episode introuvable sur la carte SD".into(),
        0x13 => "aucune entree favorite trouvee (il en faut exactement une)".into(),
        0x14 => "plusieurs entrees favorite trouvees (il en faut exactement une)".into(),
        0x15 => "echec de creation du fichier de favoris".into(),
        0x16 => "echec de lecture du fichier de favoris".into(),
        0x17 => "echec de verification finale (aucun message d'erreur specifique)".into(),
        0x18 => "erreur de nettoyage final apres mise a jour".into(),
        _ => format!("playlist rejetee, code inconnu 0x{status:02x} (structure invalide ou fichier reference manquant)"),
    }
}

pub fn send_file_status(status: u8) -> String {
    match status {
        0x00 => "pret a recevoir".into(),
        0x01 => "succes".into(),
        0x02 => "espace insuffisant sur la carte SD".into(),
        0x03 => "nom de fichier trop long".into(),
        0x04 => "SHA-256 incoherent (fichier corrompu en transit)".into(),
        0x05 => "longueur d'annonce incorrecte".into(),
        0x07 => "impossible d'ouvrir/creer le fichier sur l'enceinte".into(),
        0x08 => "erreur d'ecriture sur l'enceinte en cours de transfert".into(),
        _ => format!("code inconnu 0x{status:02x}"),
    }
}

pub fn delete_file_status(status: u8) -> String {
    if status == 0 {
        "supprime".into()
    } else {
        "erreur (fichier probablement absent)".into()
    }
}

pub fn search_file_status(status: u8) -> String {
    if status == 0 {
        "trouve".into()
    } else {
        "introuvable".into()
    }
}

pub fn download_file_status(status: u8) -> String {
    if status == 0 {
        "trouve".into()
    } else {
        "introuvable".into()
    }
}

pub fn get_file_information_status(status: u8) -> String {
    match status {
        0x00 => "succes".into(),
        0x02 => "index hors bornes".into(),
        _ => format!("code inconnu 0x{status:02x}"),
    }
}

pub fn framing_error_status(status: u8) -> String {
    match status {
        0x01 => "CRC invalide".into(),
        0x02 => "trame trop courte".into(),
        0x03 => "opcode invalide".into(),
        _ => format!("inconnu (0x{status:02x})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_playlist_known_statuses_have_specific_descriptions() {
        assert_eq!(update_playlist_status(0x00), "succes");
        assert!(update_playlist_status(0x11).contains("image"));
        assert!(update_playlist_status(0x12).contains("audio"));
        assert!(
            update_playlist_status(0x13)
                .to_lowercase()
                .contains("favorite")
        );
        assert!(
            update_playlist_status(0x14)
                .to_lowercase()
                .contains("plusieurs")
        );
    }

    #[test]
    fn update_playlist_unknown_status_falls_back_to_generic_message_containing_the_code() {
        assert!(update_playlist_status(0x99).contains("0x99"));
    }

    #[test]
    fn update_playlist_json_pipeline_statuses_are_distinct_from_missing_file_statuses() {
        let json_pipeline_codes: [u8; 5] = [0x04, 0x05, 0x06, 0x07, 0x08];
        let messages: Vec<String> = json_pipeline_codes
            .iter()
            .map(|&c| update_playlist_status(c))
            .collect();

        for message in &messages {
            assert!(
                !message.to_lowercase().contains("audio introuvable"),
                "{message} ne devrait pas parler de fichier audio manquant"
            );
        }
        assert!(messages.iter().all(|m| !m.is_empty()));
        let unique: std::collections::HashSet<&String> = messages.iter().collect();
        assert_eq!(
            unique.len(),
            json_pipeline_codes.len(),
            "chaque code doit avoir une description distincte"
        );
    }

    #[test]
    fn update_playlist_binary_post_processing_statuses() {
        assert!(
            update_playlist_status(0x0F)
                .to_lowercase()
                .contains("renommage")
        );
        assert!(update_playlist_status(0x10).to_lowercase().contains("type"));
    }

    #[test]
    fn update_playlist_image_status_covers_both_category_and_music_jpg() {
        let message = update_playlist_status(0x11).to_lowercase();
        assert!(message.contains("dossier") || message.contains("categorie"));
        assert!(message.contains("piste") || message.contains("musique"));
    }

    #[test]
    fn update_playlist_favorite_file_statuses() {
        assert!(
            update_playlist_status(0x15)
                .to_lowercase()
                .contains("favori")
        );
        assert!(
            update_playlist_status(0x16)
                .to_lowercase()
                .contains("favori")
        );
    }

    #[test]
    fn update_playlist_unlogged_and_cleanup_statuses() {
        assert!(
            update_playlist_status(0x17)
                .to_lowercase()
                .contains("aucun")
        );
        assert!(
            update_playlist_status(0x18)
                .to_lowercase()
                .contains("nettoyage")
        );
    }

    #[test]
    fn send_file_known_statuses() {
        assert_eq!(send_file_status(0x00), "pret a recevoir");
        assert_eq!(send_file_status(0x01), "succes");
        assert!(send_file_status(0x02).contains("espace"));
    }

    #[test]
    fn delete_search_download_statuses_are_binary() {
        assert_eq!(delete_file_status(0), "supprime");
        assert_ne!(delete_file_status(1), "supprime");
        assert_eq!(search_file_status(0), "trouve");
        assert_eq!(download_file_status(0), "trouve");
    }

    #[test]
    fn framing_error_statuses_are_distinct() {
        let crc = framing_error_status(0x01);
        let short = framing_error_status(0x02);
        let bad_opcode = framing_error_status(0x03);
        assert_ne!(crc, short);
        assert_ne!(short, bad_opcode);
        assert_ne!(crc, bad_opcode);
    }
}
