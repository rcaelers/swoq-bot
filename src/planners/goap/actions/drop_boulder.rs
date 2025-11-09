use crate::infra::{use_direction, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory, Tile};

use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct DropBoulderAction {
    pub drop_pos: Position, // Position where boulder will be dropped (adjacent to player)
}

impl GOAPActionTrait for DropBoulderAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Player must have boulder in inventory
        if player.inventory != Inventory::Boulder {
            return false;
        }

        // Boulder must be unexplored
        if state.player_states[player_index].boulder_is_unexplored != Some(true) {
            return false;
        }

        // Drop position must be empty and adjacent to player
        if !player.position.is_adjacent(&self.drop_pos) {
            return false;
        }

        // Drop position must be empty
        matches!(world.map.get(&self.drop_pos), Some(Tile::Empty))
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        // Drop the boulder
        state.world.players[player_index].inventory = Inventory::None;
        // Place boulder at drop position
        state.world.map.insert(self.drop_pos, Tile::Boulder);
        state.world.boulders.add_boulder(self.drop_pos, true); // Mark as moved
        // Clear boulder tracking
        state.player_states[player_index].boulder_is_unexplored = None;
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];
        let player_pos = player.position;

        // Drop the boulder in the designated direction
        let action = use_direction(player_pos, self.drop_pos);
        (action, ExecutionStatus::Complete)
    }

    fn cost(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // Low cost for dropping
        1.0
    }

    fn duration(&self, _state: &PlannerState, _player_index: usize) -> u32 {
        // Just 1 tick to drop
        1
    }

    fn name(&self) -> &'static str {
        "DropBoulder"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate if player has a boulder
        if player.inventory != Inventory::Boulder {
            return actions;
        }

        // Only drop unexplored boulders - check planner state
        if state.player_states[player_index].boulder_is_unexplored != Some(true) {
            return actions;
        }

        // Find all adjacent empty positions that wouldn't block important paths
        for &drop_pos in &player.position.neighbors() {
            // Must be empty
            if !matches!(world.map.get(&drop_pos), Some(Tile::Empty)) {
                continue;
            }

            // Check if dropping here would block paths to important locations
            let would_block = would_block_critical_path(world, drop_pos, player_index);

            if !would_block {
                let action = DropBoulderAction { drop_pos };
                if action.precondition(state, player_index) {
                    actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                }
            }
        }

        actions
    }
}

/// Check if placing a boulder at this position would block critical paths
/// Simple rule: all empty neighbors of the boulder position should remain reachable from each other
fn would_block_critical_path(world: &WorldState, boulder_pos: Position, _player_index: usize) -> bool {
    // Find all empty neighbors of the boulder position
    let empty_neighbors: Vec<Position> = boulder_pos
        .neighbors()
        .into_iter()
        .filter(|&pos| matches!(world.map.get(&pos), Some(Tile::Empty)))
        .collect();

    // If there are fewer than 2 empty neighbors, it can't block anything
    if empty_neighbors.len() < 2 {
        return false;
    }

    // Temporarily simulate boulder placement
    let mut test_world = world.clone();
    test_world.map.insert(boulder_pos, Tile::Boulder);

    // Check that all empty neighbors can still reach each other
    for i in 0..empty_neighbors.len() {
        for j in (i + 1)..empty_neighbors.len() {
            let from = empty_neighbors[i];
            let to = empty_neighbors[j];
            
            // If these neighbors were connected before but not after, it blocks
            if world.find_path(from, to).is_some() && test_world.find_path(from, to).is_none() {
                return true;
            }
        }
    }

    false
}
