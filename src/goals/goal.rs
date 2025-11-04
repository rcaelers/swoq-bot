use crate::goals::avoid_enemy::AvoidEnemyGoal;
use crate::goals::clear_path_on_goal_change;
use crate::goals::drop_boulder::DropBoulderGoal;
use crate::goals::drop_boulder_on_plate::DropBoulderOnPlateGoal;
use crate::goals::explore::ExploreGoal;
use crate::goals::fetch_boulder::FetchBoulderGoal;
use crate::goals::get_key::GetKeyGoal;
use crate::goals::kill_enemy::KillEnemyGoal;
use crate::goals::open_door::OpenDoorGoal;
use crate::goals::pass_through_door::PassThroughDoorGoal;
use crate::goals::pickup_health::PickupHealthGoal;
use crate::goals::pickup_sword::PickupSwordGoal;
use crate::goals::random_explore::RandomExploreGoal;
use crate::goals::reach_exit::ReachExitGoal;
use crate::goals::wait_on_tile::WaitOnTileGoal;
use crate::swoq_interface::DirectedAction;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

/// Trait for executing goals
pub trait ExecuteGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction>;
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
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        clear_path_on_goal_change(world, player_index, self);

        // Delegate to specific goal implementation
        match self {
            Goal::Explore => ExploreGoal.execute(world, player_index),
            Goal::GetKey(color) => GetKeyGoal(*color).execute(world, player_index),
            Goal::OpenDoor(color) => OpenDoorGoal(*color).execute(world, player_index),
            Goal::WaitOnTile(_color, pos) => WaitOnTileGoal(*pos).execute(world, player_index),
            Goal::PassThroughDoor(_color, door_pos, target_pos) => {
                PassThroughDoorGoal::new(*door_pos, *target_pos).execute(world, player_index)
            }
            Goal::PickupSword => PickupSwordGoal.execute(world, player_index),
            Goal::PickupHealth(pos) => PickupHealthGoal(*pos).execute(world, player_index),
            Goal::ReachExit => ReachExitGoal.execute(world, player_index),
            Goal::KillEnemy(pos) => KillEnemyGoal(*pos).execute(world, player_index),
            Goal::AvoidEnemy(pos) => AvoidEnemyGoal(*pos).execute(world, player_index),
            Goal::FetchBoulder(pos) => FetchBoulderGoal(*pos).execute(world, player_index),
            Goal::DropBoulderOnPlate(_color, pos) => {
                DropBoulderOnPlateGoal(*pos).execute(world, player_index)
            }
            Goal::DropBoulder => DropBoulderGoal.execute(world, player_index),
            Goal::RandomExplore(pos) => RandomExploreGoal(*pos).execute(world, player_index),
        }
    }

    /// Execute goal for a specific player (convenience wrapper)
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn execute_for_player(
        &self,
        world: &mut WorldState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        self.execute(world, player_index)
    }
}
