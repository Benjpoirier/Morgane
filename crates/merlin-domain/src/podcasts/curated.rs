use super::search::PodcastSearchResult;

struct Entry {
    title: &'static str,
    feed_url: &'static str,
    image_url: &'static str,
    episode_count: u32,
    genre: &'static str,
    author: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        title: "Une histoire et… Oli",
        feed_url: "https://radiofrance-podcast.net/podcast09/podcast_d555ed4e-dbe5-4908-912e-b3169f9ceede.xml",
        image_url: "https://www.radiofrance.fr/s3/cruiser-production-eu3/2025/10/e9ad5830-54bd-4400-b908-5a51daa2b96c/1400x1400_sc_carre-une-histoire-et-oli-generique.jpg",
        episode_count: 166,
        genre: "Dès 5 ans · Histoires",
        author: "France Inter",
    },
    Entry {
        title: "Encore une histoire",
        feed_url: "https://feeds.acast.com/public/shows/670d1795df4dd6f896655670",
        image_url: "https://assets.pippa.io/shows/670d1795df4dd6f896655670/show-cover.jpeg",
        episode_count: 574,
        genre: "Dès 4 ans · Histoires",
        author: "Encore une histoire",
    },
    Entry {
        title: "Les P'tites Histoires",
        feed_url: "https://feeds.acast.com/public/shows/5b7989f2-e269-4dd2-94c0-752f7bbc687a",
        image_url: "https://assets.pippa.io/shows/619570212eacc3a36070252b/1767609079904-4946f769-c0f7-44fc-9f18-fc1589ac53dc.jpeg",
        episode_count: 823,
        genre: "Dès 3 ans · Histoires",
        author: "Taleming",
    },
    Entry {
        title: "La grande histoire de Pomme d'Api",
        feed_url: "https://feed.ausha.co/B6r8OclKP6gn",
        image_url: "https://image.ausha.co/ZvneIxFuuDouEZTgDWn7x0K68CMVNtTEohyQDhTx_1400x1400.jpeg",
        episode_count: 205,
        genre: "Dès 3 ans · Histoires",
        author: "Bayard Jeunesse",
    },
    Entry {
        title: "Le Petit Nicolas et autres histoires cultes",
        feed_url: "https://feeds.acast.com/public/shows/679a17e101388342ba90b249",
        image_url: "https://assets.pippa.io/shows/679a17e101388342ba90b249/1780457547625-17ebdf68-0ae3-4cb2-bcfe-6d99d0fafb0d.jpeg",
        episode_count: 64,
        genre: "Dès 6 ans · Histoires cultes",
        author: "Encore une histoire",
    },
    Entry {
        title: "Les Odyssées",
        feed_url: "https://radiofrance-podcast.net/podcast09/podcast_c361798b-d6e3-4282-ba0a-ebb051b9e424.xml",
        image_url: "https://www.radiofrance.fr/s3/cruiser-production-eu3/2026/04/2bab1da1-f313-4183-88f3-d31acfdb9079/1400x1400_sc_carre-les-odyssees-avril-2026.jpg",
        episode_count: 196,
        genre: "Dès 7 ans · Histoire",
        author: "France Inter",
    },
    Entry {
        title: "Bestioles",
        feed_url: "https://radiofrance-podcast.net/podcast09/podcast_a80ecbd5-df3d-4c9d-bee7-4e3d9efc1974.xml",
        image_url: "https://www.radiofrance.fr/s3/cruiser-production-eu3/2025/12/c7b61f65-164a-4ec8-b0dd-74a887acf3bc/1400x1400_sc_carre-bestioles-novembre.jpg",
        episode_count: 121,
        genre: "Dès 5 ans · Nature",
        author: "France Inter",
    },
    Entry {
        title: "Petits Curieux",
        feed_url: "https://feeds.acast.com/public/shows/petits-curieux",
        image_url: "https://assets.pippa.io/shows/662870ac33dbf400129c0073/1713926336987-6c74b68293d634f9b9b818cdec95700d.jpeg",
        episode_count: 445,
        genre: "Dès 7 ans · Curiosité",
        author: "Choses à Savoir",
    },
    Entry {
        title: "Salut l'info !",
        feed_url: "https://radiofrance-podcast.net/podcast09/podcast_9f5486b0-38b5-4544-bf49-e24b18bd022c.xml",
        image_url: "https://www.radiofrance.fr/s3/cruiser-production-eu3/2024/07/ff2d16ff-1fb5-495d-bef8-23ab2a378fa4/1400x1400_sc_logo-sli-franceinfo-rvb.jpg",
        episode_count: 13,
        genre: "Dès 7 ans · Actu",
        author: "franceinfo × Astrapi",
    },
    Entry {
        title: "Escape News",
        feed_url: "https://anchor.fm/s/385d7e00/podcast/rss",
        image_url: "https://d3t3ozftmdmh3i.cloudfront.net/production/podcast_uploaded_nologo/9356512/9356512-1609423519813-e9ad5e2a1568e.jpg",
        episode_count: 19,
        genre: "Dès 10 ans · Actu ados",
        author: "Escape News",
    },
];

pub fn list() -> Vec<PodcastSearchResult> {
    ENTRIES
        .iter()
        .map(|e| PodcastSearchResult {
            title: e.title.to_string(),
            feed_url: e.feed_url.to_string(),
            image_url: Some(e.image_url.to_string()),
            episode_count: Some(e.episode_count),
            genre: Some(e.genre.to_string()),
            author: Some(e.author.to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_addable_and_labelled() {
        let list = list();
        assert!(list.len() >= 8);
        for entry in &list {
            assert!(
                entry.feed_url.starts_with("https://"),
                "flux non https : {}",
                entry.title
            );
            assert!(
                entry.genre.is_some(),
                "âge/thème manquant : {}",
                entry.title
            );
        }
    }

    #[test]
    fn feed_urls_are_unique() {
        let mut feeds: Vec<_> = list().into_iter().map(|e| e.feed_url).collect();
        let count = feeds.len();
        feeds.sort();
        feeds.dedup();
        assert_eq!(
            feeds.len(),
            count,
            "flux dupliqué dans la sélection curatée"
        );
    }
}
