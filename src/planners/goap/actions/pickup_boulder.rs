use crate::infra::{Color, Position, use_direction};
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupBoulderAction {
    pub boulder_pos: Position,
    pub target_pos: Position, // Adjacent position where player will stand to pick up boulder
    pub is_unexplored: bool,  // Whether this boulder has not been moved yet
    pub cached_distance: u32, // Cached path distance from generation to avoid repeated pathfinding
}

impl PickupBoulderAction {
    /// Use BFS to find reachable boulders, categorized by type
    /// Returns Vec<(boulder_pos, target_pos, distance, is_unexplored, on_plate)>
    #[tracing::instrument(skip(world))]
    fn find_boulders_bfs(
        world: &WorldState,
        start_pos: Position,
    ) -> Vec<(Position, Position, u32, bool, bool)> {
        use std::collections::{HashSet, VecDeque};

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut found_boulders = Vec::new();

        queue.push_back((start_pos, 0u32));
        visited.insert(start_pos);

        // Get all pressure plate positions
        let plate_positions: HashSet<Position> = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .filter_map(|&color| world.pressure_plates.get_positions(color))
            .flatten()
            .copied()
            .collect();

        // Track found boulders to avoid duplicates
        let mut found_boulder_positions = HashSet::new();

        while let Some((current_pos, distance)) = queue.pop_front() {
            // Check neighbors for boulders
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

            // Continue BFS to adjacent walkable tiles
            for next_pos in world.valid_neighbors(&current_pos) {
                if !visited.contains(&next_pos) && world.is_walkable(&next_pos, next_pos) {
                    visited.insert(next_pos);
                    queue.push_back((next_pos, distance + 1));
                }
            }
        }

        found_boulders
    }
}

impl GOAPActionTrait for PickupBoulderAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Player must have empty inventory and boulder must exist
        // Path reachability was already validated during generation
        player.inventory == Inventory::None && world.boulders.contains(&self.boulder_pos)
    }

    fn effect_end(&self, state: &mut GameState, player_index: usize) {
        // Simulate picking up the boulder
        state.world.players[player_index].inventory = Inventory::Boulder;
        // Mark boulder as moved (it will be in player's inventory)
        state.world.boulders.remove_boulder(&self.boulder_pos);
        // Move player to the pre-determined target position
        state.world.players[player_index].position = self.target_pos;
        // Track whether this boulder is unexplored
        state.player_states[player_index].boulder_is_unexplored = Some(self.is_unexplored);
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

        // Navigate to the target position
        execute_move_to(world, player_index, self.target_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        // Cost is based on cached distance to target position
        5.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 for picking up
    }

    fn name(&self) -> String {
        "PickupBoulder".to_string()
    }

    #[tracing::instrument(skip(state))]
    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        // Only generate from level 6 onwards
        if world.level < 6 {
            return actions;
        }

        let player = &world.players[player_index];

        // Player must have empty inventory
        if player.inventory != Inventory::None {
            return actions;
        }

        // Use single BFS to find all reachable boulders
        let all_boulders = Self::find_boulders_bfs(world, player.position);

        // Categorize boulders
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

        // Decision logic based on priority:
        // 1. If there are unexplored boulders, return closest reachable one
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
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }
        // 2. If no unexplored, return closest explored boulder not on pressure plate
        else if !explored_not_on_plate.is_empty() {
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
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }
        // 3. If none of the above, return all reachable boulders on pressure plates
        else if !explored_on_plate.is_empty() {
            for &(boulder_pos, target_pos, cached_distance, _, _) in &explored_on_plate {
                let action = PickupBoulderAction {
                    boulder_pos: *boulder_pos,
                    target_pos: *target_pos,
                    is_unexplored: false,
                    cached_distance: *cached_distance,
                };
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
