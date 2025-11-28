//! DropBoulderOnPlate action - drop a boulder on a pressure plate

use std::collections::{HashSet, VecDeque};

use crate::infra::{Color, Position, use_direction};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct DropBoulderOnPlateAction {
    pub plate_pos: Position,
    pub target_pos: Position,
    pub color: Color,
    pub cached_distance: u32,
}

impl DropBoulderOnPlateAction {
    /// Check if any player is at a position
    fn player_at(world: &WorldState, pos: &Position) -> bool {
        world.players.iter().any(|p| &p.position == pos)
    }

    /// Use BFS to find reachable plates that don't have boulders on them
    fn find_empty_plates_bfs(
        world: &WorldState,
        start_pos: Position,
    ) -> Vec<(Position, Position, Color, u32)> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut found_plates = Vec::new();

        // Get all plate positions by color
        let plates_by_color: Vec<(Color, Vec<Position>)> = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .filter_map(|&color| {
                world
                    .pressure_plates
                    .get_positions(color)
                    .map(|positions| (color, positions.iter().copied().collect()))
            })
            .collect();

        queue.push_back((start_pos, 0u32));
        visited.insert(start_pos);

        while let Some((current_pos, distance)) = queue.pop_front() {
            // Check neighbors for empty plates
            for neighbor in world.valid_neighbors(&current_pos) {
                for (color, positions) in &plates_by_color {
                    if positions.contains(&neighbor)
                        && !world.boulders.contains(&neighbor)
                        && !Self::player_at(world, &neighbor)
                    {
                        found_plates.push((neighbor, current_pos, *color, distance));
                    }
                }
            }

            // Continue BFS
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

        found_plates
    }
}

impl RLActionTrait for DropBoulderOnPlateAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must be holding a boulder
        if player.inventory != Inventory::Boulder {
            return false;
        }

        // Plate must not have a boulder or player on it
        !world.boulders.contains(&self.plate_pos) && !Self::player_at(world, &self.plate_pos)
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

        // If we're at the target position, drop the boulder on the plate
        if player_pos == self.target_pos {
            let action = use_direction(player_pos, self.plate_pos);
            return (action, ExecutionStatus::Complete);
        }

        execute_move_to(world, player_index, self.target_pos, execution_state)
    }

    fn name(&self) -> String {
        format!("DropBoulderOnPlate({:?})", self.color)
    }

    fn action_type_index(&self) -> usize {
        ActionType::DropBoulderOnPlate as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.plate_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();

        // Only generate from level 6 onwards (pressure plates appear with boulders)
        if world.level < 6 {
            return actions;
        }

        let player = &world.players[player_index];

        // Player must be holding a boulder
        if player.inventory != Inventory::Boulder {
            return actions;
        }

        // Find empty plates using BFS
        let empty_plates = Self::find_empty_plates_bfs(world, player.position);

        // Generate actions for all reachable empty plates
        for (plate_pos, target_pos, color, cached_distance) in empty_plates {
            let action = DropBoulderOnPlateAction {
                plate_pos,
                target_pos,
                color,
                cached_distance,
            };
            actions.push(Box::new(action) as Box<dyn RLActionTrait>);
        }

        actions
    }
}
