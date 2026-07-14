use merlin_domain::library::repositories::SubscriptionRepository;
use merlin_domain::library::subscription::Subscription;
use rusqlite::{Connection, params};

pub struct SqliteSubscriptionRepository {
    connection: Connection,

    device_id: String,
}

impl SqliteSubscriptionRepository {
    pub fn new(connection: Connection, device_id: impl Into<String>) -> Self {
        Self {
            connection,
            device_id: device_id.into(),
        }
    }

    pub fn claim_legacy(&self) {
        let _ = self.connection.execute(
            "UPDATE OR IGNORE subscriptions SET device_id = ?1 WHERE device_id = ''",
            params![self.device_id],
        );

        let _ = self
            .connection
            .execute("DELETE FROM subscriptions WHERE device_id = ''", []);
    }
}

fn row_to_subscription(row: &rusqlite::Row<'_>) -> rusqlite::Result<Subscription> {
    let guids_json: String = row.get("selected_episode_guids")?;
    Ok(Subscription {
        feed_url: row.get("feed_url")?,
        title: row.get("title")?,
        kind: row.get("kind")?,
        category: row.get("category")?,
        selected_episode_guids: serde_json::from_str(&guids_json).unwrap_or_default(),
        feed_image_url: row.get("feed_image_url")?,
        direct_audio_url: row.get("direct_audio_url")?,
        direct_title: row.get("direct_title")?,
        direct_image_url: row.get("direct_image_url")?,
    })
}

impl SubscriptionRepository for SqliteSubscriptionRepository {
    fn all(&self) -> Vec<Subscription> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT * FROM subscriptions WHERE device_id = ?1")
        else {
            return Vec::new();
        };
        statement
            .query_map(params![self.device_id], row_to_subscription)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn add(&mut self, subscription: Subscription) {
        let guids = serde_json::to_string(&subscription.selected_episode_guids)
            .unwrap_or_else(|_| "[]".to_string());
        let _ = self.connection.execute(
            "INSERT OR IGNORE INTO subscriptions
             (device_id, feed_url, title, kind, category, selected_episode_guids,
              feed_image_url, direct_audio_url, direct_title, direct_image_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                self.device_id,
                subscription.feed_url,
                subscription.title,
                subscription.kind,
                subscription.category,
                guids,
                subscription.feed_image_url,
                subscription.direct_audio_url,
                subscription.direct_title,
                subscription.direct_image_url,
            ],
        );
    }

    fn delete(&mut self, feed_url: &str) {
        let _ = self.connection.execute(
            "DELETE FROM subscriptions WHERE device_id = ?1 AND feed_url = ?2",
            params![self.device_id, feed_url],
        );
    }

    fn update_feed_metadata(
        &mut self,
        feed_url: &str,
        title: Option<&str>,
        feed_image_url: Option<&str>,
    ) {
        if let Some(title) = title {
            let _ = self.connection.execute(
                "UPDATE subscriptions SET title = ?3 WHERE device_id = ?1 AND feed_url = ?2",
                params![self.device_id, feed_url, title],
            );
        }
        if let Some(feed_image_url) = feed_image_url {
            let _ = self.connection.execute(
                "UPDATE subscriptions SET feed_image_url = ?3 WHERE device_id = ?1 AND feed_url = ?2",
                params![self.device_id, feed_url, feed_image_url],
            );
        }
    }

    fn update_selected_episode_guids(&mut self, feed_url: &str, guids: &[String]) {
        let json = serde_json::to_string(guids).unwrap_or_else(|_| "[]".to_string());
        let _ = self.connection.execute(
            "UPDATE subscriptions SET selected_episode_guids = ?3 WHERE device_id = ?1 AND feed_url = ?2",
            params![self.device_id, feed_url, json],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::db;

    fn make_repo() -> SqliteSubscriptionRepository {
        SqliteSubscriptionRepository::new(db::open_in_memory().expect("db"), "dev-1")
    }

    fn subscription(feed_url: &str, title: &str) -> Subscription {
        Subscription {
            title: title.to_string(),
            ..Subscription::new(feed_url)
        }
    }

    #[test]
    fn add_then_all_returns_the_same_subscription() {
        let mut repo = make_repo();
        let mut sub = subscription("https://example.com/feed", "Mon Podcast");
        sub.category = "Histoires".to_string();

        repo.add(sub.clone());

        assert_eq!(repo.all(), vec![sub]);
    }

    #[test]
    fn add_same_feed_url_twice_does_not_duplicate() {
        let mut repo = make_repo();
        repo.add(subscription("https://example.com/feed", "Premier ajout"));
        repo.add(subscription("https://example.com/feed", "Second ajout"));

        assert_eq!(
            repo.all().len(),
            1,
            "un même feedURL ne doit jamais se dupliquer"
        );
        assert_eq!(
            repo.all()[0].title,
            "Premier ajout",
            "le premier ajout gagne"
        );
    }

    #[test]
    fn delete_removes_subscription_by_feed_url() {
        let mut repo = make_repo();
        repo.add(subscription("https://a.com", "A"));
        repo.add(subscription("https://b.com", "B"));

        repo.delete("https://a.com");

        let feed_urls: Vec<String> = repo.all().into_iter().map(|s| s.feed_url).collect();
        assert_eq!(feed_urls, ["https://b.com"]);
    }

    #[test]
    fn update_feed_metadata_mutates_in_place() {
        let mut repo = make_repo();
        repo.add(subscription("https://a.com", "Ancien titre"));

        repo.update_feed_metadata(
            "https://a.com",
            Some("Nouveau titre"),
            Some("https://a.com/img.jpg"),
        );

        let updated = repo
            .all()
            .into_iter()
            .find(|s| s.feed_url == "https://a.com")
            .unwrap();
        assert_eq!(updated.title, "Nouveau titre");
        assert_eq!(
            updated.feed_image_url.as_deref(),
            Some("https://a.com/img.jpg")
        );
    }

    #[test]
    fn update_selected_episode_guids_replaces_the_list() {
        let mut repo = make_repo();
        repo.add(subscription("https://a.com", "A"));

        repo.update_selected_episode_guids("https://a.com", &["guid-1".into(), "guid-2".into()]);

        assert_eq!(repo.all()[0].selected_episode_guids, ["guid-1", "guid-2"]);
    }
}
