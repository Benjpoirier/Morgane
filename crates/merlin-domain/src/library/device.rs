#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredDevice {
    pub mac: String,

    pub name: String,

    pub is_active: bool,

    pub last_connected_at: Option<i64>,
}
