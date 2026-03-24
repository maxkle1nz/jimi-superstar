use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotBindingState {
    Empty,
    Installed,
    Active,
    Locked,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalitySlot {
    pub slot_id: String,
    pub label: String,
    pub capsule_id: Option<String>,
    pub principal_id: Option<String>,
    pub state: SlotBindingState,
    pub unlock_required: bool,
    pub active_mandala_id: Option<String>,
}
