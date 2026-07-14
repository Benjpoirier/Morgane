#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualCategory {
    pub uuid: String,
    pub title: String,
    pub image_source: String,
}
