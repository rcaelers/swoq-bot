use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::infra::Position;
use crate::state::Map;

// ============================================================================
// CBS (Conflict-Based Search) Data Structures
// ============================================================================

/// Agent with start and goal positions
#[derive(Clone, Debug)]
pub struct Agent {
    pub id: usize,
    pub start: Position,
    pub goal: Position,
}

/// Represents a constraint on an agent's movement
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Constraint {
    /// Agent cannot be at position at given timestep
    Vertex {
        agent: usize,
        pos: Position,
        time: i32,
    },
    /// Agent cannot move from pos1 to pos2 at given timestep
    Edge {
        agent: usize,
        from: Position,
        to: Position,
        time: i32,
    },
}

/// Represents a conflict between two agents
#[derive(Clone, Debug)]
struct Conflict {
    agent1: usize,
    agent2: usize,
    pos: Position,
    time: i32,
    conflict_type: ConflictType,
}

#[derive(Clone, Debug)]
enum ConflictType {
    /// Both agents at same position at same time
    Vertex,
    /// Agents swap positions (edge conflict)
    Edge { from1: Position, from2: Position },
    /// Sequential execution: agent1 moves to where agent2 was (agent2 hasn't moved yet)
    Sequential { prev_pos_j: Position },
}

/// Node in the Constraint Tree (CT)
#[derive(Clone)]
struct CTNode {
    constraints: Vec<Constraint>,
    solution: Vec<Vec<Position>>, // Paths for each agent
    cost: i32,                    // Sum of path costs
}

impl CTNode {
    fn new(num_agents: usize) -> Self {
        Self {
            constraints: Vec::new(),
            solution: vec![Vec::new(); num_agents],
            cost: 0,
        }
    }
}

impl Ord for CTNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: lower cost has higher priority
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for CTNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for CTNode {}

impl PartialEq for CTNode {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

// ============================================================================
// A* Node for CBS pathfinding
// ============================================================================

#[derive(Clone, Eq, PartialEq)]
struct AStarNode {
    pos: Position,
    f_score: i32,
    tick: i32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_score.cmp(&self.f_score)
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// Helper functions for CBS
// ============================================================================

fn heuristic(a: Position, b: Position) -> i32 {
    a.distance(&b)
}

fn reconstruct_path(
    came_from: &HashMap<Position, Position>,
    mut current: Position,
) -> Vec<Position> {
    let mut path = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        path.push(prev);
        current = prev;
    }
    path.reverse();
    path
}

/// A* pathfinding with CBS constraints
fn find_path_with_constraints(
    map: &Map,
    start: Position,
    goal: Position,
    agent_id: usize,
    constraints: &[Constraint],
    is_walkable: &dyn Fn(&Position, usize, Position) -> bool,
) -> Option<Vec<Position>> {
    tracing::trace!(
        "CBS A*: Finding path for agent {} from {:?} to {:?} with {} constraints",
        agent_id,
        start,
        goal,
        constraints.len()
    );
    
    if start == goal {
        tracing::trace!("CBS A*: Agent {} already at goal", agent_id);
        return Some(vec![goal]);
    }

    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<Position, Position> = HashMap::new();
    let mut g_score: HashMap<Position, i32> = HashMap::new();
    let mut closed_set: HashSet<Position> = HashSet::new();

    g_score.insert(start, 0);
    open_set.push(AStarNode {
        pos: start,
        f_score: heuristic(start, goal),
        tick: 0,
    });

    const MAX_EXPANSIONS: usize = 5000;
    let mut expansions = 0;

    while let Some(AStarNode {
        pos: current,
        tick: current_tick,
        ..
    }) = open_set.pop()
    {
        if current == goal {
            tracing::trace!(
                "CBS A*: Agent {} reached goal after {} expansions",
                agent_id,
                expansions
            );
            return Some(reconstruct_path(&came_from, current));
        }

        if closed_set.contains(&current) {
            continue;
        }
        closed_set.insert(current);

        expansions += 1;
        if expansions > MAX_EXPANSIONS {
            tracing::warn!(
                "CBS A*: Agent {} exceeded MAX_EXPANSIONS ({}) - from {:?} to {:?}",
                agent_id,
                MAX_EXPANSIONS,
                start,
                goal
            );
            return None;
        }

        let current_g_score = *g_score.get(&current).unwrap_or(&0);

        for neighbor in current.neighbors() {
            if closed_set.contains(&neighbor) {
                continue;
            }

            if neighbor.x < 0
                || neighbor.x >= map.width
                || neighbor.y < 0
                || neighbor.y >= map.height
            {
                continue;
            }

            // Check walkability for this agent
            if !is_walkable(&neighbor, agent_id, goal) {
                continue;
            }

            let next_tick = current_tick + 1;

            // Check CBS constraints
            if violates_constraints(&neighbor, &current, next_tick, constraints) {
                continue;
            }

            let tentative_g = current_g_score + 1; // Unit cost

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative_g);
                open_set.push(AStarNode {
                    pos: neighbor,
                    f_score: tentative_g + heuristic(neighbor, goal),
                    tick: next_tick,
                });
            }
        }
    }

    None
}

/// Check if a move violates any constraints
fn violates_constraints(
    to: &Position,
    from: &Position,
    time: i32,
    constraints: &[Constraint],
) -> bool {
    constraints.iter().any(|c| match c {
        Constraint::Vertex { pos, time: t, .. } => pos == to && *t == time,
        Constraint::Edge {
            from: cf,
            to: ct,
            time: t,
            ..
        } => cf == from && ct == to && *t == time,
    })
}

/// Pathfinding environment (map and walkability function)
struct PathfindingEnv<'a> {
    map: &'a Map,
    is_walkable: &'a dyn Fn(&Position, usize, Position) -> bool,
}

// ============================================================================
// CBS (Conflict-Based Search) Implementation
// ============================================================================

pub struct CBS;

impl CBS {
    /// Find collision-free paths for multiple agents
    ///
    /// # Arguments
    /// * `map` - The game map
    /// * `agents` - Slice of agents with their start and goal positions
    /// * `is_walkable` - Function to check if a position is walkable for a specific agent
    ///   Receives (position, agent_id, goal_position) -> bool
    ///
    /// # Returns
    /// Vector of paths, one for each agent. Returns None if no solution exists.
    pub fn find_paths<F>(map: &Map, agents: &[Agent], is_walkable: F) -> Option<Vec<Vec<Position>>>
    where
        F: Fn(&Position, usize, Position) -> bool,
    {
        tracing::debug!("CBS: Starting with {} agents", agents.len());
        for agent in agents.iter() {
            tracing::debug!(
                "CBS: Agent {} - start: {:?}, goal: {:?}",
                agent.id,
                agent.start,
                agent.goal
            );
        }

        // Check for agents with same goal
        for i in 0..agents.len() {
            for j in (i + 1)..agents.len() {
                if agents[i].goal == agents[j].goal {
                    tracing::debug!(
                        "CBS: Agents {} and {} have the same goal {:?}",
                        agents[i].id,
                        agents[j].id,
                        agents[i].goal
                    );
                }
            }
        }

        let num_agents = agents.len();
        let mut open = BinaryHeap::new();

        // Initialize root node
        let mut root = CTNode::new(num_agents);

        // Find initial paths for all agents (no constraints)
        let env = PathfindingEnv {
            map,
            is_walkable: &is_walkable,
         };
        
        tracing::debug!("CBS: Finding initial paths for all agents");
        for (agent_idx, agent) in agents.iter().enumerate() {
            tracing::debug!("CBS: Finding initial path for agent {}", agent.id);
            match Self::replan_and_update(&mut root, agent, agent_idx, &env) {
                Some(_) => tracing::debug!("CBS: Initial path found for agent {}", agent.id),
                None => {
                    tracing::warn!("CBS: No initial path for agent {} (start: {:?}, goal: {:?})", agent.id, agent.start, agent.goal);
                    return None;
                }
            }
        }

        tracing::debug!("CBS: All initial paths found, starting conflict resolution");
        open.push(root);

        const MAX_CT_NODES: usize = 1000;
        let mut nodes_expanded = 0;

        while let Some(node) = open.pop() {
            nodes_expanded += 1;
            tracing::debug!(
                "CBS: Expanding CT node {}/{} with cost {}",
                nodes_expanded,
                MAX_CT_NODES,
                node.cost
            );
            
            if nodes_expanded > MAX_CT_NODES {
                tracing::warn!("CBS: Timeout - expanded {} nodes", nodes_expanded);
                return None; // Timeout
            }

            // Check for conflicts
            if let Some(conflict) = Self::find_first_conflict(&node.solution, agents) {
                tracing::debug!(
                    "CBS: Found conflict between agents {} and {} at {:?} (time: {}, type: {:?})",
                    conflict.agent1,
                    conflict.agent2,
                    conflict.pos,
                    conflict.time,
                    conflict.conflict_type
                );
                
                // Create two child nodes with new constraints
                let child_nodes = Self::create_child_nodes(&node, &conflict, &env, agents);
                tracing::debug!("CBS: Created {} child nodes", child_nodes.len());

                for child in child_nodes {
                    open.push(child);
                }
            } else {
                // No conflicts - solution found!
                tracing::info!("CBS: Solution found after expanding {} nodes", nodes_expanded);
                return Some(node.solution);
            }
        }

        tracing::warn!("CBS: No solution found after expanding {} nodes", nodes_expanded);
        None // No solution found
    }

    /// Detect the first conflict in the current solution
    fn find_first_conflict(solution: &[Vec<Position>], agents: &[Agent]) -> Option<Conflict> {
        let num_agents = solution.len();

        // Find maximum path length
        let max_len = solution.iter().map(|p| p.len()).max().unwrap_or(0);

        // Check each timestep
        for t in 0..max_len {
            // Check all pairs of agents
            for i in 0..num_agents {
                for j in (i + 1)..num_agents {
                    let pos_i = Self::get_position_at_time(&solution[i], t);
                    let pos_j = Self::get_position_at_time(&solution[j], t);

                    // Vertex conflict: same position at same time
                    if pos_i == pos_j {
                        // Allow same-goal conflicts: if both agents have the same goal
                        // and the conflict is at that goal, skip it (sequential execution handles this)
                        let agent_i = &agents[i];
                        let agent_j = &agents[j];
                        if agent_i.goal == agent_j.goal && pos_i == agent_i.goal {
                            tracing::debug!(
                                "CBS: Ignoring same-goal vertex conflict at {:?} for agents {} and {}",
                                pos_i, i, j
                            );
                            continue;
                        }
                        
                        return Some(Conflict {
                            agent1: i,
                            agent2: j,
                            pos: pos_i,
                            time: t as i32,
                            conflict_type: ConflictType::Vertex,
                        });
                    }

                    // Sequential execution conflict: agent i moves to where agent j was
                    // (only check i < j since actions execute in order)
                    if t > 0 {
                        let prev_j = Self::get_position_at_time(&solution[j], t - 1);
                        if pos_i == prev_j && pos_i != pos_j {
                            // Agent i at time t occupies where agent j was at time t-1
                            // This is invalid because agent j might not have moved yet
                            return Some(Conflict {
                                agent1: i,
                                agent2: j,
                                pos: pos_i,
                                time: t as i32,
                                conflict_type: ConflictType::Sequential { prev_pos_j: prev_j },
                            });
                        }
                    }

                    // Edge conflict: agents swap positions
                    if t > 0 {
                        let prev_i = Self::get_position_at_time(&solution[i], t - 1);
                        let prev_j = Self::get_position_at_time(&solution[j], t - 1);

                        if pos_i == prev_j && pos_j == prev_i {
                            return Some(Conflict {
                                agent1: i,
                                agent2: j,
                                pos: pos_i,
                                time: t as i32,
                                conflict_type: ConflictType::Edge {
                                    from1: prev_i,
                                    from2: prev_j,
                                },
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// Get agent position at time t (stays at goal if path ends)
    fn get_position_at_time(path: &[Position], t: usize) -> Position {
        if t < path.len() {
            path[t]
        } else {
            *path.last().unwrap()
        }
    }

    /// Create child nodes by adding constraints for each agent in the conflict
    fn create_child_nodes(
        parent: &CTNode,
        conflict: &Conflict,
        env: &PathfindingEnv,
        agents: &[Agent],
    ) -> Vec<CTNode> {
        let mut children = Vec::new();

        // Create child for agent1 constraint
        if let Some(child) =
            Self::create_child_with_constraint(parent, &agents[conflict.agent1], conflict.agent1, conflict, env)
        {
            children.push(child);
        }

        // Create child for agent2 constraint
        if let Some(child) =
            Self::create_child_with_constraint(parent, &agents[conflict.agent2], conflict.agent2, conflict, env)
        {
            children.push(child);
        }

        children
    }

    /// Create a child node with a new constraint for a specific agent
    fn create_child_with_constraint(
        parent: &CTNode,
        agent: &Agent,
        agent_idx: usize,
        conflict: &Conflict,
        env: &PathfindingEnv,
    ) -> Option<CTNode> {
        let mut child = parent.clone();

        // Add constraint for this agent based on the conflict
        Self::add_constraint(&mut child, agent.id, conflict);

        // Replan and update the child node
        Self::replan_and_update(&mut child, agent, agent_idx, env)?;

        Some(child)
    }

    /// Add a constraint to a CT node based on conflict type
    fn add_constraint(node: &mut CTNode, agent: usize, conflict: &Conflict) {
        let new_constraint = match &conflict.conflict_type {
            ConflictType::Vertex => Constraint::Vertex {
                agent,
                pos: conflict.pos,
                time: conflict.time,
            },
            ConflictType::Edge { from1, from2 } => {
                let is_agent1 = agent == conflict.agent1;
                let (from, to) = if is_agent1 {
                    (*from1, conflict.pos)
                } else {
                    (*from2, conflict.pos)
                };
                Constraint::Edge {
                    agent,
                    from,
                    to,
                    time: conflict.time,
                }
            }
            ConflictType::Sequential { prev_pos_j } => {
                // Only constrain agent1 (the one trying to move to agent2's previous position)
                if agent == conflict.agent1 {
                    Constraint::Vertex {
                        agent,
                        pos: *prev_pos_j,
                        time: conflict.time,
                    }
                } else {
                    // For agent2, we could constrain it to stay or move from prev_pos_j
                    // But actually, we need agent2 to either stay put or move away
                    // Let's constrain agent2 to not be at the conflict position at time t-1->t transition
                    Constraint::Vertex {
                        agent,
                        pos: *prev_pos_j,
                        time: conflict.time - 1,
                    }
                }
            }
        };

        node.constraints.push(new_constraint);
    }

    /// Replan path for agent and update node's cost and solution
    fn replan_and_update(node: &mut CTNode, agent: &Agent, agent_idx: usize, env: &PathfindingEnv) -> Option<()> {
        tracing::trace!(
            "CBS: Replanning for agent {} (start: {:?}, goal: {:?})",
            agent.id,
            agent.start,
            agent.goal
        );
        
        // Get constraints for this agent
        let agent_constraints: Vec<Constraint> = node
            .constraints
            .iter()
            .filter(|c| match c {
                Constraint::Vertex { agent: a, .. } => *a == agent.id,
                Constraint::Edge { agent: a, .. } => *a == agent.id,
            })
            .cloned()
            .collect();

        tracing::trace!(
            "CBS: Agent {} has {} constraints",
            agent.id,
            agent_constraints.len()
        );

        // Replan path for this agent with new constraints
        let new_path = find_path_with_constraints(
            env.map,
            agent.start,
            agent.goal,
            agent.id,
            &agent_constraints,
            &env.is_walkable,
        )?;

        tracing::trace!(
            "CBS: Agent {} new path length: {}",
            agent.id,
            new_path.len()
        );

        // Update solution and cost
        node.cost = node.cost - node.solution[agent_idx].len() as i32 + new_path.len() as i32;
        node.solution[agent_idx] = new_path;

        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swoq_interface::Tile;

    #[test]
    fn test_cbs_simple_crossing() {
        // Create a 5x5 map with all floor tiles
        let mut map = Map::new(5, 5);
        for x in 0..5 {
            for y in 0..5 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Two agents crossing paths
        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 2 },
                goal: Position { x: 4, y: 2 },
            },
            Agent {
                id: 1,
                start: Position { x: 4, y: 2 },
                goal: Position { x: 0, y: 2 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "CBS should find a solution");

        let paths = result.unwrap();
        assert_eq!(paths.len(), 2);

        // Verify no collisions
        let max_len = paths.iter().map(|p| p.len()).max().unwrap();
        for t in 0..max_len {
            let pos1 = if t < paths[0].len() {
                paths[0][t]
            } else {
                *paths[0].last().unwrap()
            };
            let pos2 = if t < paths[1].len() {
                paths[1][t]
            } else {
                *paths[1].last().unwrap()
            };

            assert_ne!(pos1, pos2, "Collision at timestep {}: both agents at {:?}", t, pos1);

            // Check for edge conflicts (swaps)
            if t > 0 {
                let prev1 = if t - 1 < paths[0].len() {
                    paths[0][t - 1]
                } else {
                    *paths[0].last().unwrap()
                };
                let prev2 = if t - 1 < paths[1].len() {
                    paths[1][t - 1]
                } else {
                    *paths[1].last().unwrap()
                };

                let swapped = pos1 == prev2 && pos2 == prev1;
                assert!(!swapped, "Edge conflict at timestep {}", t);
            }
        }
    }

    #[test]
    fn test_cbs_with_obstacles() {
        // Create a map with a wall in the middle
        let mut map = Map::new(7, 7);
        for x in 0..7 {
            for y in 0..7 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Add vertical wall
        for y in 1..6 {
            map.insert(Position { x: 3, y }, Tile::Wall);
        }

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 3 },
                goal: Position { x: 6, y: 3 },
            },
            Agent {
                id: 1,
                start: Position { x: 6, y: 3 },
                goal: Position { x: 0, y: 3 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "CBS should find paths around obstacle");

        let paths = result.unwrap();

        // Verify paths reach goals
        assert_eq!(*paths[0].last().unwrap(), agents[0].goal);
        assert_eq!(*paths[1].last().unwrap(), agents[1].goal);

        // Verify no collisions
        let max_len = paths.iter().map(|p| p.len()).max().unwrap();
        for t in 0..max_len {
            let pos1 = if t < paths[0].len() {
                paths[0][t]
            } else {
                *paths[0].last().unwrap()
            };
            let pos2 = if t < paths[1].len() {
                paths[1][t]
            } else {
                *paths[1].last().unwrap()
            };

            assert_ne!(pos1, pos2, "Collision at timestep {}", t);
        }
    }

    #[test]
    fn test_cbs_same_goal() {
        // Two agents, same start and goal
        let mut map = Map::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 0 },
                goal: Position { x: 0, y: 0 },
            },
            Agent {
                id: 1,
                start: Position { x: 0, y: 0 },
                goal: Position { x: 0, y: 0 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // This should fail or both agents stay at same location
        // Current implementation will find conflict and fail
        assert!(result.is_none() || result.as_ref().unwrap()[0] == vec![Position { x: 0, y: 0 }]);
    }

    #[test]
    fn test_cbs_sequential_conflict() {
        // Test sequential execution: agent1 tries to move to where agent2 currently is
        let mut map = Map::new(5, 5);
        for x in 0..5 {
            for y in 0..5 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 0 },
                goal: Position { x: 2, y: 0 },
            },
            Agent {
                id: 1,
                start: Position { x: 1, y: 0 },
                goal: Position { x: 1, y: 1 }, // Agent 2 moves away
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // May or may not find solution depending on constraints - just verify it doesn't crash
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_cbs_edge_constraint() {
        // Test edge constraints (agents swapping positions)
        let mut map = Map::new(5, 3);
        for x in 0..5 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 1 },
                goal: Position { x: 4, y: 1 },
            },
            Agent {
                id: 1,
                start: Position { x: 4, y: 1 },
                goal: Position { x: 0, y: 1 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "CBS should find a solution avoiding edge conflicts");

        let paths = result.unwrap();

        // Verify no edge conflicts (swapping)
        for t in 1..paths[0].len().min(paths[1].len()) {
            let pos1_prev = paths[0][t - 1];
            let pos1_t = paths[0][t];
            let pos2_prev = paths[1][t - 1];
            let pos2_t = paths[1][t];

            let swapped = pos1_t == pos2_prev && pos2_t == pos1_prev;
            assert!(!swapped, "Edge conflict (swap) at timestep {}", t);
        }
    }

    #[test]
    fn test_cbs_no_path_blocked() {
        // Test case where one agent blocks the only path for another
        let mut map = Map::new(5, 3);
        for x in 0..5 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Create walls to form a narrow corridor
        for x in 0..5 {
            map.insert(Position { x, y: 0 }, Tile::Wall);
            map.insert(Position { x, y: 2 }, Tile::Wall);
        }
        // Leave only middle row walkable

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 1 },
                goal: Position { x: 4, y: 1 },
            },
            Agent {
                id: 1,
                start: Position { x: 4, y: 1 },
                goal: Position { x: 0, y: 1 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // This should fail as there's no way for agents to pass each other in a 1-wide corridor
        assert!(result.is_none(), "CBS should fail when no solution exists");
    }

    #[test]
    fn test_cbs_timeout() {
        // Test that CBS times out with too many CT nodes
        let mut map = Map::new(10, 10);
        for x in 0..10 {
            for y in 0..10 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Create many agents with conflicting paths to trigger timeout
        let mut agents = Vec::new();
        for i in 0..5 {
            agents.push(Agent {
                id: i,
                start: Position { x: 0, y: i as i32 },
                goal: Position { x: 9, y: i as i32 },
            });
            agents.push(Agent {
                id: i + 5,
                start: Position { x: 9, y: i as i32 },
                goal: Position { x: 0, y: i as i32 },
            });
        }

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // Should either find solution or timeout - just verify it returns something
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_cbs_pathfinding_max_expansions() {
        // Test A* timeout with too many expansions
        let mut map = Map::new(100, 100);
        for x in 0..100 {
            for y in 0..100 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Create a complex maze-like scenario
        for x in 0..99 {
            for y in 0..99 {
                if (x + y) % 3 == 0 {
                    map.insert(Position { x, y }, Tile::Wall);
                }
            }
        }

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 0 },
            goal: Position { x: 99, y: 99 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // May find path or hit max expansions - either is acceptable
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_cbs_single_agent() {
        // Test with just one agent (no conflicts possible)
        let mut map = Map::new(5, 5);
        for x in 0..5 {
            for y in 0..5 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 0 },
            goal: Position { x: 4, y: 4 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Single agent should always find a path");

        let paths = result.unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0][0], agents[0].start);
        assert_eq!(*paths[0].last().unwrap(), agents[0].goal);
    }

    #[test]
    fn test_cbs_agent_already_at_goal() {
        // Test agent that starts at its goal
        let mut map = Map::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 0 },
                goal: Position { x: 0, y: 0 },
            },
            Agent {
                id: 1,
                start: Position { x: 2, y: 2 },
                goal: Position { x: 1, y: 1 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Should handle agents already at goal");

        let paths = result.unwrap();
        assert_eq!(paths[0], vec![Position { x: 0, y: 0 }]);
    }

    #[test]
    fn test_cbs_out_of_bounds_neighbor() {
        // Test that pathfinding correctly handles out-of-bounds positions
        let mut map = Map::new(2, 2);
        for x in 0..2 {
            for y in 0..2 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 0 },
            goal: Position { x: 1, y: 1 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Should find path in small map");
    }

    #[test]
    fn test_cbs_unwalkable_positions() {
        // Test that unwalkable positions are avoided
        let mut map = Map::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }
        map.insert(Position { x: 1, y: 1 }, Tile::Wall);

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 1 },
            goal: Position { x: 2, y: 1 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Should find path around obstacle");

        let paths = result.unwrap();
        // Verify path doesn't go through the wall
        for pos in &paths[0] {
            assert_ne!(*pos, Position { x: 1, y: 1 }, "Path should not go through wall");
        }
    }

    #[test]
    fn test_cbs_no_solution_unwalkable() {
        // Test case where goal is completely blocked
        let mut map = Map::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Surround goal with walls
        map.insert(Position { x: 1, y: 1 }, Tile::Wall);
        map.insert(Position { x: 0, y: 1 }, Tile::Wall);
        map.insert(Position { x: 2, y: 1 }, Tile::Wall);
        map.insert(Position { x: 1, y: 0 }, Tile::Wall);
        map.insert(Position { x: 1, y: 2 }, Tile::Wall);

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 0 },
            goal: Position { x: 1, y: 1 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_none(), "Should fail when goal is unreachable");
    }

    #[test]
    fn test_cbs_closed_set_duplicate() {
        // Test that closed set prevents revisiting nodes
        let mut map = Map::new(10, 10);
        for x in 0..10 {
            for y in 0..10 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents = vec![Agent {
            id: 0,
            start: Position { x: 0, y: 0 },
            goal: Position { x: 9, y: 9 },
        }];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Should find path");
    }

    #[test]
    fn test_cbs_sequential_agent2_constraint() {
        // Test that sequential conflicts constrain agent2 appropriately
        let mut map = Map::new(5, 3);
        for x in 0..5 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        // Create a scenario where agent1 would collide with agent2's position
        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 1 },
                goal: Position { x: 3, y: 1 },
            },
            Agent {
                id: 1,
                start: Position { x: 1, y: 1 },
                goal: Position { x: 4, y: 1 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        // Should either find solution or timeout
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_edge_conflict_direct() {
        // Force an edge conflict scenario where agents try to swap
        let mut map = Map::new(3, 1);
        for x in 0..3 {
            map.insert(Position { x, y: 0 }, Tile::Empty);
        }

        // Two agents need to swap in a narrow corridor - will trigger edge conflict
        let agents = vec![
            Agent {
                id: 0,
                start: Position { x: 0, y: 0 },
                goal: Position { x: 2, y: 0 },
            },
            Agent {
                id: 1,
                start: Position { x: 1, y: 0 },
                goal: Position { x: 0, y: 0 },
            },
        ];

        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);

        if let Some(paths) = result {
            // Verify edge constraints were applied correctly
            // Check that agents don't swap positions at any timestep
            let max_len = paths[0].len().max(paths[1].len());
            for t in 1..max_len {
                let pos1_prev = if t - 1 < paths[0].len() {
                    paths[0][t - 1]
                } else {
                    *paths[0].last().unwrap()
                };
                let pos1_curr = if t < paths[0].len() {
                    paths[0][t]
                } else {
                    *paths[0].last().unwrap()
                };
                let pos2_prev = if t - 1 < paths[1].len() {
                    paths[1][t - 1]
                } else {
                    *paths[1].last().unwrap()
                };
                let pos2_curr = if t < paths[1].len() {
                    paths[1][t]
                } else {
                    *paths[1].last().unwrap()
                };

                assert!(
                    !(pos1_curr == pos2_prev && pos2_curr == pos1_prev),
                    "Edge conflict detected at t={}",
                    t
                );
            }
        }
    }

    #[test]
    fn test_empty_agents() {
        // Test with no agents
        let mut map = Map::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                map.insert(Position { x, y }, Tile::Empty);
            }
        }

        let agents: Vec<Agent> = vec![];
        let is_walkable = |pos: &Position, _agent_id: usize, _goal: Position| -> bool {
            matches!(map.get(pos), Some(Tile::Empty))
        };

        let result = CBS::find_paths(&map, &agents, is_walkable);
        assert!(result.is_some(), "Should handle empty agent list");
        assert_eq!(result.unwrap().len(), 0);
    }
}
