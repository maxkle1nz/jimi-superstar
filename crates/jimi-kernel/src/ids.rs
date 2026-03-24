use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaneId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(format!("sess_{}", Uuid::now_v7()))
    }
}

impl LaneId {
    pub fn new() -> Self {
        Self(format!("lane_{}", Uuid::now_v7()))
    }
}

impl TurnId {
    pub fn new() -> Self {
        Self(format!("turn_{}", Uuid::now_v7()))
    }
}
