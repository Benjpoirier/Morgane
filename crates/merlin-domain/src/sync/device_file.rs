#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFile {
    pub name: String,
    pub size: usize,
}

impl DeviceFile {
    pub fn new(name: impl Into<String>, size: usize) -> Self {
        Self {
            name: name.into(),
            size,
        }
    }
}

pub fn is_protected_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "playlist.bin" | "playlist.json" | "playlist.txt" | "manifest.json"
    ) || lower.ends_with(".cfg")
        || lower.ends_with(".json")
}

pub fn is_credential_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(lower.as_str(), "wifi.cfg" | "sta_wifi.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_protected_and_secret() {
        for name in ["wifi.cfg", "STA_WIFI.JSON", "sta_wifi.json"] {
            assert!(is_credential_file(name), "{name} est un secret");
            assert!(is_protected_file(name), "{name} est aussi protege");
        }
    }

    #[test]
    fn system_files_are_protected_but_not_secret() {
        for name in [
            "playlist.bin",
            "playlist.json",
            "sleep.cfg",
            "manifest.json",
        ] {
            assert!(is_protected_file(name), "{name} est systeme");
            assert!(!is_credential_file(name), "{name} n'est pas un secret");
        }
    }

    #[test]
    fn content_files_are_neither() {
        for name in [
            "3c8cc39a-2a47-4f63-ac3c-8da64ed18acd.mp3",
            "abc.aac",
            "cat-1.jpg",
        ] {
            assert!(!is_protected_file(name), "{name} est du contenu");
            assert!(!is_credential_file(name));
        }
    }
}
