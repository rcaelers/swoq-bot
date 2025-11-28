//! PickupBoulder action - pick up a boulder from the ground

use std::collections::{HashSet, VecDeque};

use crate::infra::{use_direction, Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct PickupBoulderAction {
    pub boulder_pos: Position,
    pub target_pos: Position,
    pub is_unexplored: bool,
    pub cached_distance: u32,
}

impl PickupBoulderAction {
    /// Use BFS to find reachable boulders
    fn find_boulders_bfs(
        world: &WorldState,
        start_pos: Position,
    ) -> Vec<(Position, Position, u32, bool, bool)> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut found_boulders = Vec::new();

        queue.push_back((start_pos, 0u32));
        visited.insert(start_pos);

        let plate_positions: HashSet<Position> = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .filter_map(|&color| world.pressure_plates.get_positions(color))
            .flatten()
            .copied()
            .collect();

        let mut found_boulder_positions = HashSet::new();

        while let Some((current_pos, distance)) = queue.pop_front() {
            for neighbor in world.valid_neighbors(&current_pos) {
                if world.boulders.contains(&neighbor)
                    && !found_boulder_positions.contains(&neighbor)
                {
                    found_boulder_positions.insert(neighbor);
                    let is_unexplored = !world.boulders.has_moved(&neighbor);
                    let on_plate = plate_positions.contains(&neighbor);
                    found_boulders.push((neighbor, current_pos, distance, is_unexplored, on_plate));
                }
            }

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

        found_boulders
    }
}

impl RLActionTrait for PickupBoulderAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must have empty inventory
        if player.inventory != Inventory::None {
            return false;
        }

        // Boulder must exist
        world.boulders.contains(&self.boulder_pos)
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

        // If we're at the target position, pick up the boulder
        if player_pos == self.target_pos {
            let action = use_direction(player_pos, self.boulder_pos);
            return (action, ExecutionStatus::Complete);
        }

        execute_move_to(world, player_index, self.target_pos, execution_state)
    }

    fn name(&self) -> String {
        "PickupBoulder".to_string()
    }

    fn action_type_index(&self) -> usize {
        ActionType::PickupBoulder as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.boulder_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();

        // Only generate from level 6 onwards
        if world.level < 6 {
            return actions;
        }

        let player = &world.players[player_index];

        // Player must have empty inventory
        if player.inventory != Inventory::None {
            return actions;
        }

        let all_boulders = Self::find_boulders_bfs(world, player.position);

        let unexplored: Vec<_> = all_boulders
            .iter()
            .filter(|(_, _, _, is_unexplored, _)| *is_unexplored)
            .collect();

        let explored_not_on_plate: Vec<_> = all_boulders
            .iter()
            .filter(|(_, _, _, is_unexplored, on_plate)| !*is_unexplored && !*on_plate)
            .collect();

        let explored_on_plate: Vec<_> = all_boulders
            .iter()
            .filter(|(_, _, _, is_unexplored, on_plate)| !*is_unexplored && *on_plate)
            .collect();

        if !unexplored.is_empty() {
            if let Some(&(boulder_pos, target_pos, cached_distance, _, _)) =
                unexplored.iter().min_by_key(|(_, _, dist, _, _)| *dist)
            {
                let action = PickupBoulderAction {
                    boulder_pos: *boulder_pos,
                    target_pos: *target_pos,
                    is_unexplored: true,
                    cached_distance: *cached_distance,
                };
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        } else if !explored_not_on_plate.is_empty() {
            if let Some(&(boulder_pos, target_pos, cached_distance, _, _)) = explored_not_on_plate
                .iter()
                .min_by_key(|(_, _, dist, _, _)| *dist)
            {
                let action = PickupBoulderAction {
                    boulder_pos: *boulder_pos,
                    target_pos: *target_pos,
                    is_unexplored: false,
                    cached_distance: *cached_distance,
                };
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        } else if !explored_on_plate.is_empty() {
            for &(boulder_pos, target_pos, cached_distance, _, _) in &explored_on_plate {
                let action = PickupBoulderAction {
                    boulder_pos: *boulder_pos,
                    target_pos: *target_pos,
                    is_unexplored: false,
                    cached_distance: *cached_distance,
                };
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        }

        actions
    }
}
