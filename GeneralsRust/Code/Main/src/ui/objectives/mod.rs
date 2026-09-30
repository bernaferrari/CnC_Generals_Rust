pub use generals_presentation::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};

/// UI collection wrapper; the objective values themselves are shared contract data.
#[derive(Debug, Default, Clone)]
pub struct MissionObjectivesUI {
    pub objectives: Vec<ObjectiveDisplay>,
}
