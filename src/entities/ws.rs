#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub event: String,
    pub payload: String,
}
