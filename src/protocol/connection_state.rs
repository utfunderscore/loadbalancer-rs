
#[derive(Debug)]
pub enum ConnectionState {
    HandShake,
    Status,
    Login,
    Transfer,
    Config,
    Play,
}

impl ConnectionState {
    pub fn from_intent(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::Status),
            2 => Some(Self::Login),
            3 => Some(Self::Transfer),
            _ => None,
        }
    }
}
