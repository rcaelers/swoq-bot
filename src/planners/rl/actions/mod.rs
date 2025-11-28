//! Simplified action trait for RL - removes planning-specific methods from GOAPActionTrait

mod attack_enemy;
mod avoid_enemy;
mod drop_boulder;
mod drop_boulder_on_plate;
mod explore;
mod get_key;
pub mod helpers;
mod hunt_enemy;
mod open_door;
mod pass_through_door_with_plate;
mod pickup_boulder;
mod pickup_health;
mod pickup_sword;
mod reach_exit;
mod touch_plate;
mod wait;
mod wait_on_plate;

pub use attack_enemy::AttackEnemyAction;
pub use avoid_enemy::AvoidEnemyAction;
pub use drop_boulder::DropBoulderAction;
pub use drop_boulder_on_plate::DropBoulderOnPlateAction;
pub use explore::ExploreAction;
pub use get_key::GetKeyAction;
pub use hunt_enemy::HuntEnemyAction;
pub use open_door::OpenDoorAction;
pub use pass_through_door_with_plate::PassThroughDoorWithPlateAction;
pub use pickup_boulder::PickupBoulderAction;
pub use pickup_health::PickupHealthAction;
pub use pickup_sword::PickupSwordAction;
pub use reach_exit::ReachExitAction;
pub use touch_plate::TouchPlateAction;
pub use wait::WaitAction;
pub use wait_on_plate::WaitOnPlateAction;

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

/// Simplified trait for RL actions - only includes methods needed for action generation and execution.
/// Removes planning-specific methods: cost(), duration(), reward(), effect_start(), effect_end()
pub trait RLActionTrait: std::fmt::Debug + RLActionClone + Send + Sync {
    /// Check if this action can be executed in the current state
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool;

    /// Prepare phase: Set destination for CBS pathfinding
    /// Returns the destination position this action wants to reach, or None for stationary actions
    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        None
    }

    /// Execute the action, returning the low-level action and status
    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus);

    /// Human-readable name for logging/debugging
    fn name(&self) -> String;

    /// Returns true if this action should terminate the episode for this player
    fn is_terminal(&self) -> bool {
        false
    }

    /// Returns true if this is a combat-related action
    fn is_combat_action(&self) -> bool {
        false
    }

    /// Generate all possible instances of this action type based on current state
    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>>
    where
        Self: Sized;

    /// Get the action type index for encoding (0-17 for the 18 action types)
    fn action_type_index(&self) -> usize;

    /// Get the target position for this action (if applicable)
    fn target_position(&self) -> Option<Position> {
        None
    }
}

/// Helper trait for cloning trait objects
pub trait RLActionClone {
    fn clone_box(&self) -> Box<dyn RLActionTrait>;
}

impl<T> RLActionClone for T
where
    T: 'static + RLActionTrait + Clone,
{
    fn clone_box(&self) -> Box<dyn RLActionTrait> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn RLActionTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Execution status for multi-tick actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Action is still in progress
    InProgress,
    /// Action completed successfully
    Complete,
    /// Action failed and cannot continue
    Failed,
    /// Action is waiting for a precondition
    Wait,
}

/// State for tracking multi-tick action execution
#[derive(Debug, Clone, Default)]
pub struct ActionExecutionState {
    pub exploration_target: Option<Position>,
    pub hunt_target: Option<Position>,
    pub initial_object_counts: Option<ObjectCounts>,
    pub wait_ticks: u32,
    pub enemy_under_attack: Option<Position>,
    pub phase_complete: bool,
}

/// Object counts for exploration discovery tracking
#[derive(Debug, Clone, Default)]
pub struct ObjectCounts {
    pub num_keys: usize,
    pub num_swords: usize,
    pub num_health: usize,
    pub num_pressure_plates: usize,
    pub num_boulders: usize,
    pub exit_visible: bool,
}

/// Action type enumeration for encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    Explore = 0,
    GetKey = 1,
    OpenDoor = 2,
    PickupSword = 3,
    PickupHealth = 4,
    AttackEnemy = 5,
    HuntEnemy = 6,
    AvoidEnemy = 7,
    WaitOnPlate = 8,
    PassThroughDoorWithPlate = 9,
    PickupBoulder = 10,
    DropBoulder = 11,
    DropBoulderOnPlate = 12,
    TouchPlate = 13,
    ReachExit = 14,
    Wait = 15,
}

impl ActionType {
    pub const COUNT: usize = 16;

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(ActionType::Explore),
            1 => Some(ActionType::GetKey),
            2 => Some(ActionType::OpenDoor),
            3 => Some(ActionType::PickupSword),
            4 => Some(ActionType::PickupHealth),
            5 => Some(ActionType::AttackEnemy),
            6 => Some(ActionType::HuntEnemy),
            7 => Some(ActionType::AvoidEnemy),
            8 => Some(ActionType::WaitOnPlate),
            9 => Some(ActionType::PassThroughDoorWithPlate),
            10 => Some(ActionType::PickupBoulder),
            11 => Some(ActionType::DropBoulder),
            12 => Some(ActionType::DropBoulderOnPlate),
            13 => Some(ActionType::TouchPlate),
            14 => Some(ActionType::ReachExit),
            15 => Some(ActionType::Wait),
            _ => None,
        }
    }
}
