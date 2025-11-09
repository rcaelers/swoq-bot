use crate::planners::heuristic::goals::avoid_enemy::AvoidEnemyGoal;
use crate::planners::heuristic::goals::clear_path_on_goal_change;
use crate::planners::heuristic::goals::drop_boulder::DropBoulderGoal;
use crate::planners::heuristic::goals::drop_boulder_on_plate::DropBoulderOnPlateGoal;
use crate::planners::heuristic::goals::explore::ExploreGoal;
use crate::planners::heuristic::goals::fetch_boulder::FetchBoulderGoal;
use crate::planners::heuristic::goals::get_key::GetKeyGoal;
use crate::planners::heuristic::goals::kill_enemy::KillEnemyGoal;
use crate::planners::heuristic::goals::open_door::OpenDoorGoal;
use crate::planners::heuristic::goals::pass_through_door::PassThroughDoorGoal;
use crate::planners::heuristic::goals::pickup_health::PickupHealthGoal;
use crate::planners::heuristic::goals::pickup_sword::PickupSwordGoal;
use crate::planners::heuristic::goals::random_explore::RandomExploreGoal;
use crate::planners::heuristic::goals::reach_exit::ReachExitGoal;
use crate::planners::heuristic::goals::wait_on_tile::WaitOnTileGoal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;
use crate::infra::{Color, Position};

/// Trait for executing goals
pub trait ExecuteGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Explore,
    GetKey(Color),
    OpenDoor(Color),
    WaitOnTile(Color, Position),
    PassThroughDoor(Color, Position, Position), // door_pos, target_pos (beyond door)
    PickupSword,
    PickupHealth(Position),
    AvoidEnemy(Position),
    KillEnemy(Position),
    FetchBoulder(Position),
    DropBoulder,
    DropBoulderOnPlate(Color, Position),
    ReachExit,
    RandomExplore(Position),
}

impl Goal {
    #[tracing::instrument(level = "debug", skip(state))]
    pub fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        clear_path_on_goal_change(state, player_index, self);

        // Delegate to specific goal implementation
        match self {
            Goal::Explore => ExploreGoal.execute(state, player_index),
            Goal::GetKey(color) => GetKeyGoal(*color).execute(state, player_index),
            Goal::OpenDoor(color) => OpenDoorGoal(*color).execute(state, player_index),
            Goal::WaitOnTile(_color, pos) => WaitOnTileGoal(*pos).execute(state, player_index),
            Goal::PassThroughDoor(_color, door_pos, target_pos) => {
                PassThroughDoorGoal::new(*door_pos, *target_pos).execute(state, player_index)
            }
            Goal::PickupSword => PickupSwordGoal.execute(state, player_index),
            Goal::PickupHealth(pos) => PickupHealthGoal(*pos).execute(state, player_index),
            Goal::ReachExit => ReachExitGoal.execute(state, player_index),
            Goal::KillEnemy(pos) => KillEnemyGoal(*pos).execute(state, player_index),
            Goal::AvoidEnemy(pos) => AvoidEnemyGoal(*pos).execute(state, player_index),
            Goal::FetchBoulder(pos) => FetchBoulderGoal(*pos).execute(state, player_index),
            Goal::DropBoulderOnPlate(_color, pos) => {
                DropBoulderOnPlateGoal(*pos).execute(state, player_index)
            }
            Goal::DropBoulder => DropBoulderGoal.execute(state, player_index),
            Goal::RandomExplore(pos) => RandomExploreGoal(*pos).execute(state, player_index),
        }
    }

    /// Execute goal for a specific player (convenience wrapper)
    #[tracing::instrument(level = "debug", skip(state))]
    pub fn execute_for_player(
        &self,
        state: &mut PlannerState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        self.execute(state, player_index)
    }

    /// Format goal as a display string for UI and logging
    pub fn to_display_string(&self) -> String {
        match self {
            Goal::Explore => "Explore".to_string(),
            Goal::GetKey(color) => format!("GetKey({:?})", color),
            Goal::OpenDoor(color) => format!("OpenDoor({:?})", color),
            Goal::WaitOnTile(color, _pos) => format!("WaitOnTile({:?})", color),
            Goal::PassThroughDoor(color, _door_pos, _target_pos) => {
                format!("PassDoor({:?})", color)
            }
            Goal::PickupSword => "PickupSword".to_string(),
            Goal::PickupHealth(_pos) => "PickupHealth".to_string(),
            Goal::AvoidEnemy(_pos) => "AvoidEnemy".to_string(),
            Goal::KillEnemy(_pos) => "KillEnemy".to_string(),
            Goal::FetchBoulder(_pos) => "FetchBoulder".to_string(),
            Goal::DropBoulder => "DropBoulder".to_string(),
            Goal::DropBoulderOnPlate(color, _pos) => format!("DropOnPlate({:?})", color),
            Goal::ReachExit => "ReachExit".to_string(),
            Goal::RandomExplore(_pos) => "RandomExplore".to_string(),
        }
    }
}
