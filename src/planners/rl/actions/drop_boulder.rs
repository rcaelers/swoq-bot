//! DropBoulder action - drop a boulder on an empty cell (not on a plate)

use std::collections::{HashSet, VecDeque};

use crate::infra::{use_direction, Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct DropBoulderAction {
    pub drop_pos: Position,
    pub target_pos: Position,
    pub cached_distance: u32,
}

impl DropBoulderAction {
    /// Use BFS to find valid drop positions for a boulder
    fn find_drop_positions_bfs(world: &WorldState, start_pos: Position) -> Vec<(Position, Position, u32)> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut drop_positions = Vec::new();

        // Collect plate positions to avoid
        let plate_positions: HashSet<Position> = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .filter_map(|&color| world.pressure_plates.get_positions(color))
            .flatten()
            .copied()
            .collect();

        queue.push_back((start_pos, 0u32));
        visited.insert(start_pos);

        while let Some((current_pos, distance)) = queue.pop_front() {
            // Check neighbors for valid drop positions
            for neighbor in world.valid_neighbors(&current_pos) {
                // Check if this is a valid drop position (walkable, not a plate, not occupied by boulder)
                let dummy_goal = Position::new(i32::MAX, i32::MAX);
                if world.is_walkable(&neighbor, Some(dummy_goal))
                    && !plate_positions.contains(&neighbor)
                    && !world.boulders.contains(&neighbor)
                    && !visited.contains(&neighbor)
                {
                    drop_positions.push((neighbor, current_pos, distance));
                }
            }

            // Continue BFS to find more positions
            for next_pos in world.valid_neighbors(&current_pos) {
                if !visited.contains(&next_pos) {
                    let dummy_goal = Position::new(i32::MAX, i32::MAX);
                    if world.is_walkable(&next_pos, Some(dummy_goal)) {
                        visited.insert(next_pos);
                        queue.push_back((next_pos, distance + 1));
                    }
                }
            }
        }

        drop_positions
    }
}

impl RLActionTrait for DropBoulderAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must be holding a boulder
        if player.inventory != Inventory::Boulder {
            return false;
        }

        // Drop position must be walkable and empty
        let dummy_goal = Position::new(i32::MAX, i32::MAX);
        world.is_walkable(&self.drop_pos, Some(dummy_goal))
            && !world.boulders.contains(&self.drop_pos)
    }

    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        Some(self.target_pos)
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];
        let player_pos = player.position;

        // If we're at the target position, drop the boulder
        if player_pos == self.target_pos {
            let action = use_direction(player_pos, self.drop_pos);
            return (action, ExecutionStatus::Complete);
        }

        execute_move_to(world, player_index, self.target_pos, execution_state)
    }

    fn name(&self) -> String {
        "DropBoulder".to_string()
    }

    fn action_type_index(&self) -> usize {
        ActionType::DropBoulder as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.drop_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();

        // Only generate from level 6 onwards
        if world.level < 6 {
            return actions;
        }

        let player = &world.players[player_index];

        // Player must be holding a boulder
        if player.inventory != Inventory::Boulder {
            return actions;
        }

        // Find valid drop positions using BFS
        let drop_positions = Self::find_drop_positions_bfs(world, player.position);

        // Take the closest drop position
        if let Some((drop_pos, target_pos, cached_distance)) =
            drop_positions.into_iter().min_by_key(|(_, _, dist)| *dist)
        {
            let action = DropBoulderAction {
                drop_pos,
                target_pos,
                cached_distance,
            };
            actions.push(Box::new(action) as Box<dyn RLActionTrait>);
        }

        actions
    }
}
