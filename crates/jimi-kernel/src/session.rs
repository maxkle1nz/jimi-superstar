use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    New,
    Active,
    Paused,
    Archived,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneState {
    Created,
    Focused,
    Running,
    Blocked,
    Paused,
    Merging,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Queued,
    Grounding,
    Routing,
    Executing,
    AwaitingApproval,
    Streaming,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
