use sha1::{Digest, Sha1};
use uuid::Uuid;

pub fn from_prefixed_name(name: &str) -> String {
    let digest = Sha1::digest(name.as_bytes());
    let mut bytes: [u8; 16] = digest[..16]
        .try_into()
        .expect("SHA1 fait au moins 16 octets");
    bytes[6] = (bytes[6] & 0x0F) | 0x50;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    Uuid::from_bytes(bytes).hyphenated().to_string()
}

pub fn from_guid_namespace(name: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, name.as_bytes())
        .hyphenated()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_uuid_golden_values() {
        assert_eq!(
            from_guid_namespace("guid-1"),
            "ffc2af1b-eb85-50e9-9447-2498a81d33b6"
        );
        assert_eq!(
            from_guid_namespace("https://example.com/feed/episode-1"),
            "e13c40aa-ac8b-5583-867c-62bfb245c1f9"
        );
        assert_eq!(
            from_guid_namespace(""),
            "1b4db7eb-4057-5ddf-91e0-36dec72071f5"
        );
    }

    #[test]
    fn prefixed_name_golden_values() {
        assert_eq!(
            from_prefixed_name("merlinsync-favorites"),
            "7ba21c45-fd7e-548b-b82d-3fcb09cc65fb"
        );
        assert_eq!(
            from_prefixed_name("merlinsync-category:Histoires"),
            "ddfdcfb0-631f-50ae-b373-9e175321bb96"
        );
        assert_eq!(
            from_prefixed_name("merlinsync-category:Documentaires"),
            "edecc3c2-4ea0-530a-8ecf-b6b462c047fb"
        );
        assert_eq!(
            from_prefixed_name("merlinsync-group:https://feed.example/rss:Chapitre"),
            "486b2bc2-7f23-5573-836b-3a04ac5212d0"
        );
        assert_eq!(
            from_prefixed_name("merlinsync-podcast:https://feed.example/rss"),
            "382d65b5-06b3-5933-9d77-afdffe19b1a4"
        );
        assert_eq!(
            from_prefixed_name("merlinsync-fichiers-retrouves"),
            "49351c05-c3b7-54cf-9500-6df5027c0743"
        );
    }

    #[test]
    fn the_two_algorithms_are_not_interchangeable() {
        let name = "merlinsync-favorites";
        assert_ne!(from_prefixed_name(name), from_guid_namespace(name));
    }

    #[test]
    fn uuid_is_stable_across_calls() {
        assert_eq!(
            from_guid_namespace("stable-check"),
            from_guid_namespace("stable-check")
        );
        assert_eq!(
            from_prefixed_name("stable-check"),
            from_prefixed_name("stable-check")
        );
    }

    #[test]
    fn output_is_lowercase_hyphenated() {
        let value = from_prefixed_name("Test");
        assert_eq!(value, value.to_lowercase());
        assert_eq!(value.len(), 36);
        assert_eq!(value.matches('-').count(), 4);
    }
}
