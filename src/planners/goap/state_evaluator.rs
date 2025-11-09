use crate::planners::goap::planner_state::PlannerState;
use crate::swoq_interface::Inventory;

/// Evaluate the reward/score of a world state
/// This compares the current state to determine progress toward goals
pub fn evaluate_state(state: &PlannerState, initial_state: &PlannerState) -> f32 {
    let mut score = 0.0;

    // Goal: Reach the exit with all players (ultimate goal)
    if state.world.exit_position.is_some() {
        let all_at_exit = state.world.players.iter().all(|p| {
            Some(p.position) == state.world.exit_position && p.inventory == Inventory::None // Must have empty inventory
        });
        if all_at_exit {
            score += 1000.0; // Massive reward for winning
        }
    }

    // Goal: Kill all enemies
    let enemies_killed = initial_state.world.enemies.get_positions().len() as i32
        - state.world.enemies.get_positions().len() as i32;
    if enemies_killed > 0 {
        score += enemies_killed as f32 * 30.0; // High reward per enemy killed
    }

    // Goal: Open doors (permanent progress)
    let mut doors_opened = 0;
    for color in [
        crate::infra::Color::Red,
        crate::infra::Color::Green,
        crate::infra::Color::Blue,
    ] {
        let initial_door_count = initial_state
            .world
            .doors
            .get_positions(color)
            .map(|p| p.len())
            .unwrap_or(0);
        let current_door_count = state
            .world
            .doors
            .get_positions(color)
            .map(|p| p.len())
            .unwrap_or(0);

        if !state.world.is_door_open(color) && initial_door_count > current_door_count {
            doors_opened += initial_door_count - current_door_count;
        }
    }
    if doors_opened > 0 {
        score += doors_opened as f32 * 25.0; // High reward for opening doors
    }

    // Goal: Place boulders on pressure plates (level 6+ puzzle solving)
    if state.world.level >= 6 {
        let plates_with_boulders = state
            .world
            .get_boulders_on_plates()
            .values()
            .map(|v| v.len())
            .sum::<usize>();
        let initial_plates_with_boulders = initial_state
            .world
            .get_boulders_on_plates()
            .values()
            .map(|v| v.len())
            .sum::<usize>();

        let new_plates_covered = plates_with_boulders as i32 - initial_plates_with_boulders as i32;
        if new_plates_covered > 0 {
            score += new_plates_covered as f32 * 50.0; // Very high reward for solving pressure plate puzzles
        }

        // Goal: Move unexplored boulders (discovering what's behind them)
        let unexplored_boulders = state.world.boulders.get_original_boulders().len();
        let initial_unexplored_boulders =
            initial_state.world.boulders.get_original_boulders().len();
        let boulders_explored = initial_unexplored_boulders as i32 - unexplored_boulders as i32;
        if boulders_explored > 0 {
            score += boulders_explored as f32 * 10.0; // Reward for moving unexplored boulders
        }
    }

    // Goal: Discover new areas (exploration)
    // Exploration is rewarded through discovering new objects (keys, swords, etc.)
    // which are tracked below

    // Goal: Discover new objects (keys, swords, etc.)
    let new_keys =
        count_total_keys(&state.world) as i32 - count_total_keys(&initial_state.world) as i32;
    if new_keys > 0 {
        score += new_keys as f32 * 5.0; // Reward for discovering keys
    }

    let new_swords = state.world.swords.get_positions().len() as i32
        - initial_state.world.swords.get_positions().len() as i32;
    if new_swords > 0 {
        score += new_swords as f32 * 5.0; // Reward for discovering swords
    }

    // Small reward for idle activity (touching plates when nothing else to do)
    // Only counts once per color
    let new_plate_colors_touched = state.plates_touched.len() as i32 - initial_state.plates_touched.len() as i32;
    if new_plate_colors_touched > 0 {
        score += new_plate_colors_touched as f32 * 2.0; // Small reward to encourage idle exploration
    }

    // Disqualify plans where players end with non-empty inventory
    let holding_items = state
        .world
        .players
        .iter()
        .filter(|p| p.inventory != Inventory::None)
        .count();
    if holding_items > 0 {
        return f32::NEG_INFINITY; // Never select plans with occupied inventory
    }

    score
}

fn count_total_keys(world: &crate::state::WorldState) -> usize {
    [
        crate::infra::Color::Red,
        crate::infra::Color::Green,
        crate::infra::Color::Blue,
    ]
    .iter()
    .filter_map(|color| world.keys.get_positions(*color))
    .map(|positions| positions.len())
    .sum()
}
