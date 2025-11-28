use crate::infra::{Color, Position};
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PassThroughDoorWithPlateAction {
    pub door_color: Color,
    pub door_pos: Position,
    pub wait_pos: Position, // Position to wait at (adjacent to door, on player's side)
    pub target_pos: Position, // Position to end up at (opposite side of door from wait_pos)
    pub plate_pos: Position,
}

impl GOAPActionTrait for PassThroughDoorWithPlateAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
        // PassThroughDoorWithPlateAction is only for two-player mode
        if !world.is_two_player_mode() {
            tracing::trace!(
                player_index = player_index,
                "PassThroughDoorWithPlateAction precondition failed: not in two-player mode"
            );
            return false;
        }

        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));

        tracing::trace!(
            player_index = player_index,
            door_color = ?self.door_color,
            door_pos = ?self.door_pos,
            plate_pos = ?self.plate_pos,
            door_exists = door_exists,
            plate_exists = plate_exists,
            "PassThroughDoorWithPlateAction precondition check"
        );
        // Check if other player is committed to the pressure plate:
        // - During planning: check resource claims
        // - During execution: check if already on plate or heading there (current_destination is set at runtime)
        let other_player_index = 1 - player_index;
        let other_player_committed = if other_player_index < world.players.len() {
            // Check if other player has claimed this pressure plate color during planning
            let claim = ResourceClaim::PressurePlate(self.door_color);
            state
                .resource_claims
                .get(&claim)
                .is_some_and(|&claimer| claimer == other_player_index)
        } else {
            false
        };

        tracing::trace!(
            player_index = player_index,
            other_player_index = other_player_index,
            other_player_committed = other_player_committed,
            "PassThroughDoorWithPlateAction other player commitment check"
        );

        door_exists && plate_exists && other_player_committed
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].position = self.target_pos;
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player_pos = world.players[player_index].position;

        // Check runtime conditions: door/plate still exist
        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));

        if !door_exists || !plate_exists {
            return None;
        }

        // Store our target position so WaitOnPlateAction can read it from the other player
        world.players[player_index].coop_door_target = Some(self.target_pos);

        // If door is already open, we can path directly to the target
        if world.is_door_open(self.door_color)
            && world.find_path(player_pos, self.target_pos).is_some()
        {
            return Some(self.target_pos);
        }

        // Door is closed - go to wait_pos (already validated during generate)
        if world.find_path(player_pos, self.wait_pos).is_some() {
            Some(self.wait_pos)
        } else {
            None
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player_pos = world.players[player_index].position;

        // Check if we're at the target position
        if player_pos == self.target_pos {
            // Wait here until the plate player has left the plate
            let other_player_index = 1 - player_index;
            if other_player_index < world.players.len() {
                let other_player = &world.players[other_player_index];

                // Check if other player is still on the plate
                if other_player.position == self.plate_pos {
                    // Wait for them to leave
                    return (DirectedAction::None, ExecutionStatus::InProgress);
                }
            }

            // Other player has left the plate - we can complete
            world.players[player_index].current_path = None;
            world.players[player_index].current_destination = None;
            world.players[player_index].coop_door_target = None; // Clear the coordination state
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        // Check if we're on the door tile itself - move to target
        if player_pos == self.door_pos {
            let dx = self.target_pos.x - player_pos.x;
            let dy = self.target_pos.y - player_pos.y;

            let action = if dy < 0 {
                DirectedAction::MoveNorth
            } else if dy > 0 {
                DirectedAction::MoveSouth
            } else if dx > 0 {
                DirectedAction::MoveEast
            } else if dx < 0 {
                DirectedAction::MoveWest
            } else {
                DirectedAction::None
            };

            return (action, ExecutionStatus::InProgress);
        }

        // Check if we're adjacent to the door (at the wait position set by prepare)
        if player_pos.is_adjacent(&self.door_pos) {
            // We're next to the door - check if it's open
            if world.is_door_open(self.door_color) {
                // Door is open! Move onto the door tile
                let dx = self.door_pos.x - player_pos.x;
                let dy = self.door_pos.y - player_pos.y;

                let action = if dy < 0 {
                    DirectedAction::MoveNorth
                } else if dy > 0 {
                    DirectedAction::MoveSouth
                } else if dx > 0 {
                    DirectedAction::MoveEast
                } else if dx < 0 {
                    DirectedAction::MoveWest
                } else {
                    DirectedAction::None
                };

                return (action, ExecutionStatus::InProgress);
            } else {
                // Door is closed - wait for it to open
                return (DirectedAction::None, ExecutionStatus::Wait);
            }
        }

        // Not adjacent to door yet - follow path to wait position
        execute_move_to(world, player_index, self.wait_pos, execution_state)
    }

    fn cost(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> f32 {
        10.0 + world
            .path_distance(world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn reward(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        25.0
    }

    fn duration(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> u32 {
        // Distance to door + distance through door to target + coordination overhead
        let to_door = world
            .path_distance(world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as u32;
        let through_door = self.door_pos.distance(&self.target_pos) as u32;
        to_door + through_door + 3 // +3 ticks for coordination
    }

    fn name(&self) -> String {
        "OpenDoorCoop".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        // Get other player's position for reachability check
        let other_player_index = 1 - player_index;
        let other_player_pos = if other_player_index < world.players.len() {
            Some(world.players[other_player_index].position)
        } else {
            None
        };

        // Simple check: if both players can reach each other, they're on the same side
        // and don't need door coordination
        let players_can_reach_each_other = if let Some(other_pos) = other_player_pos {
            let p = world.find_path(player.position, other_pos).is_some();
            tracing::trace!(
                player_index = player_index,
                other_player_index = other_player_index,
                players_can_reach_each_other = p,
                "PassThroughDoorWithPlateAction reachability check"
            );
            p
        } else {
            false
        };

        // Only skip if players can reach each other AND all exploration is complete
        // If there are unexplored areas, we might need plate coordination to access them
        let has_unexplored = !player.unexplored_frontier.is_empty();
        if !players_can_reach_each_other || has_unexplored {
            tracing::trace!(
                player_index = player_index,
                "Skipping PassThroughDoorWithPlateAction - players can reach each other and unexplored areas"
            );
            return actions;
        }

        for color in [
            crate::infra::Color::Red,
            crate::infra::Color::Green,
            crate::infra::Color::Blue,
        ] {
            if let (Some(door_positions), Some(plate_positions)) =
                (world.doors.get_positions(color), world.pressure_plates.get_positions(color))
            {
                for door_pos in door_positions {
                    for plate_pos in plate_positions {
                        tracing::trace!(
                            player_index = player_index,
                            door_color = ?color,
                            door_pos = ?door_pos,
                            plate_pos = ?plate_pos,
                            "Generating PassThroughDoorWithPlateAction"
                        );
                        // For each door, find reachable wait positions (adjacent to door)
                        // and pair with opposite target positions
                        let adjacent_pairs = [
                            // (wait_pos, target_pos) - opposite sides of the door
                            (
                                Position::new(door_pos.x - 1, door_pos.y),
                                Position::new(door_pos.x + 1, door_pos.y),
                            ), // west -> east
                            (
                                Position::new(door_pos.x + 1, door_pos.y),
                                Position::new(door_pos.x - 1, door_pos.y),
                            ), // east -> west
                            (
                                Position::new(door_pos.x, door_pos.y - 1),
                                Position::new(door_pos.x, door_pos.y + 1),
                            ), // north -> south
                            (
                                Position::new(door_pos.x, door_pos.y + 1),
                                Position::new(door_pos.x, door_pos.y - 1),
                            ), // south -> north
                        ];

                        for (wait_pos, target_pos) in adjacent_pairs {
                            // Don't generate if player is already at wait_pos or target_pos
                            // This prevents going back through a door we just came through
                            // (after passing through, player is at target_pos, or could be at wait_pos
                            //  which is the "opposite side" for the reverse direction)
                            if player.position == target_pos || player.position == wait_pos {
                                continue;
                            }

                            // Validate both positions are in bounds
                            if wait_pos.x < 0
                                || wait_pos.x >= world.map.width
                                || wait_pos.y < 0
                                || wait_pos.y >= world.map.height
                            {
                                tracing::trace!(
                                    player_index = player_index,
                                    wait_pos = ?wait_pos,
                                    "Skipping out-of-bounds wait_pos"
                                );
                                continue;
                            }
                            if target_pos.x < 0
                                || target_pos.x >= world.map.width
                                || target_pos.y < 0
                                || target_pos.y >= world.map.height
                            {
                                tracing::trace!(
                                    player_index = player_index,
                                    target_pos = ?target_pos,
                                    "Skipping out-of-bounds target_pos"
                                );
                                continue;
                            }

                            // wait_pos must be walkable and reachable
                            if !world.is_walkable(&wait_pos, None) {
                                tracing::trace!(
                                    player_index = player_index,
                                    wait_pos = ?wait_pos,
                                    "Skipping non-walkable wait_pos"
                                );
                                continue;
                            }
                            if world.find_path(player.position, wait_pos).is_none() {
                                tracing::trace!(
                                    player_index = player_index,
                                    wait_pos = ?wait_pos,
                                    "Skipping unreachable wait_pos"
                                );
                                continue;
                            }

                            // Only generate if the door is actually blocking us:
                            // wait_pos is reachable BUT target_pos is NOT reachable
                            // If we can already reach target_pos, there's no point in using the door
                            if world.find_path(player.position, target_pos).is_some() {
                                tracing::trace!(
                                    player_index = player_index,
                                    target_pos = ?target_pos,
                                    "Skipping - target already reachable without door"
                                );
                                continue;
                            }

                            let action = PassThroughDoorWithPlateAction {
                                door_color: color,
                                door_pos: *door_pos,
                                wait_pos,
                                target_pos,
                                plate_pos: *plate_pos,
                            };
                            if action.precondition(world, state, player_index) {
                                tracing::trace!(
                                    player_index = player_index,
                                    door_color = ?color,
                                    door_pos = ?door_pos,
                                    wait_pos = ?wait_pos,
                                    target_pos = ?target_pos,
                                    plate_pos = ?plate_pos,
                                    "Adding PassThroughDoorWithPlateAction"
                                );
                                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                            }
                        }
                    }
                }
            }
        }

        actions
    }
}
