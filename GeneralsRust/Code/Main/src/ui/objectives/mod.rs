#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectiveStatus {
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectiveCategory {
    Primary,
    Secondary,
    Bonus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObjectiveDisplay {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
    pub status: ObjectiveStatus,
    pub progress: Option<(u32, u32)>,
    pub category: ObjectiveCategory,
}

impl ObjectiveDisplay {
    pub fn new(
        id: Option<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        category: ObjectiveCategory,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            status: ObjectiveStatus::Active,
            progress: None,
            category,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MissionObjectivesUI {
    pub objectives: Vec<ObjectiveDisplay>,
}
