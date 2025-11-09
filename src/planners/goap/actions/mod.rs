mod attack_enemy;
mod avoid_enemy;
mod drop_boulder;
mod drop_boulder_on_plate;
mod explore;
mod get_key;
pub mod helpers;
mod open_door;
mod pass_through_door_with_plate;
mod pickup_boulder;
mod pickup_sword;
mod reach_exit;
mod touch_plate;
mod wait_on_plate;

pub use attack_enemy::AttackEnemyAction;
pub use drop_boulder::DropBoulderAction;
pub use drop_boulder_on_plate::DropBoulderOnPlateAction;
pub use explore::ExploreAction;
pub use get_key::GetKeyAction;
pub use open_door::OpenDoorAction;
pub use pass_through_door_with_plate::PassThroughDoorWithPlateAction;
pub use pickup_boulder::PickupBoulderAction;
pub use pickup_sword::PickupSwordAction;
pub use reach_exit::ReachExitAction;
pub use touch_plate::TouchPlateAction;
pub use wait_on_plate::WaitOnPlateAction;

use crate::infra::{Color, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

/// Trait for GOAP actions defining their preconditions, effects, and execution.
pub trait GOAPActionTrait: std::fmt::Debug + GOAPActionClone {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool;
    fn effect(&self, state: &mut PlannerState, player_index: usize);
    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus);
    fn cost(&self, state: &PlannerState, player_index: usize) -> f32;
    fn name(&self) -> &'static str;

    /// Returns the expected duration in ticks for this action to complete
    fn duration(&self, state: &PlannerState, player_index: usize) -> u32;

    /// Returns true if this action should terminate planning for this player
    /// Terminal actions prevent further expansion of the plan branch
    fn is_terminal(&self) -> bool {
        false
    }

    fn is_pass_through_door_with_plate(&self) -> Option<(Color, Position, Position)> {
        None
    }

    /// Generate all possible instances of this action type based on current state
    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>>
    where
        Self: Sized;
}

/// Helper trait for cloning trait objects
pub trait GOAPActionClone {
    fn clone_box(&self) -> Box<dyn GOAPActionTrait>;
}

impl<T> GOAPActionClone for T
where
    T: 'static + GOAPActionTrait + Clone,
{
    fn clone_box(&self) -> Box<dyn GOAPActionTrait> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn GOAPActionTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    InProgress,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct ActionExecutionState {
    pub cached_path: Option<Vec<Position>>,
    pub path_target: Option<Position>,
    pub exploration_target: Option<Position>,
    pub initial_object_counts: Option<ObjectCounts>,
    pub wait_ticks: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ObjectCounts {
    pub num_keys: usize,
    pub num_swords: usize,
    pub num_health: usize,
    pub num_pressure_plates: usize,
    pub num_boulders: usize,
    pub exit_visible: bool,
}
