use std::collections::HashMap;

use crate::infra::{Bounds, Color, Position};
use crate::state::Map;
use crate::swoq_interface::Tile;

/// Generic tracker for simple items (non-colored)
#[derive(Clone, Debug)]
pub struct ItemTracker {
    positions: Vec<Position>,
}

impl ItemTracker {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
        }
    }

    /// Update positions: merge newly seen items, deduplicate, and remove items that are gone
    /// Only validates items within any of the visibility bounds
    #[tracing::instrument(level = "trace", skip(self, map, validator, all_visibility_bounds), fields(seen_count = seen_items.len()))]
    pub fn update<F>(
        &mut self,
        seen_items: Vec<Position>,
        map: &Map,
        validator: F,
        all_visibility_bounds: &[Bounds],
    ) where
        F: Fn(&Tile) -> bool,
    {
        // Add newly seen items
        self.positions.extend(seen_items);

        // Remove duplicates manually
        let mut unique_positions: Vec<Position> = Vec::new();
        for &pos in self.positions.iter() {
            if !unique_positions.contains(&pos) {
                unique_positions.push(pos);
            }
        }
        self.positions = unique_positions;

        // Remove items that have been consumed or destroyed
        // Only check items within any player's visibility range
        self.positions.retain(|pos| {
            let is_visible = all_visibility_bounds
                .iter()
                .any(|bounds| bounds.contains(pos));
            if is_visible {
                // We can see this position (by at least one player), so check if item is still there
                if let Some(tile) = map.get(pos) {
                    validator(tile)
                } else {
                    true // Keep if we haven't seen this position
                }
            } else {
                // Out of all visibility ranges - keep the item (items don't move/disappear when not visible)
                true
            }
        });
    }

    pub fn get_positions(&self) -> &[Position] {
        &self.positions
    }

    pub fn closest_to(&self, reference: Position) -> Option<Position> {
        self.positions
            .iter()
            .min_by_key(|pos| reference.distance(pos))
            .copied()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

impl Default for ItemTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic tracker for colored items (keys, doors, pressure plates)
#[derive(Clone, Debug)]
pub struct ColoredItemTracker {
    positions: HashMap<Color, Vec<Position>>,
}

impl ColoredItemTracker {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Update positions for a specific color: merge newly seen items, deduplicate, and remove items that are gone
    /// Only validates items within any of the visibility bounds
    #[tracing::instrument(level = "trace", skip(self, map, validator, all_visibility_bounds))]
    pub fn update<F>(
        &mut self,
        seen_items: HashMap<Color, Vec<Position>>,
        map: &Map,
        validator: F,
        all_visibility_bounds: &[Bounds],
    ) where
        F: Fn(&Tile) -> bool,
    {
        self.update_with_positions(
            seen_items,
            map,
            |tile, _pos, _color| validator(tile),
            all_visibility_bounds,
        );
    }

    /// Update positions with a validator that can also check the position and color
    /// This is useful for cases like pressure plates where a player standing on them shouldn't remove them,
    /// or for doors where validation depends on the door's color
    #[tracing::instrument(level = "trace", skip(self, map, validator, all_visibility_bounds))]
    pub fn update_with_positions<F>(
        &mut self,
        seen_items: HashMap<Color, Vec<Position>>,
        map: &Map,
        validator: F,
        all_visibility_bounds: &[Bounds],
    ) where
        F: Fn(&Tile, &Position, Color) -> bool,
    {
        // Merge newly seen items with previously known ones
        for (color, new_positions) in seen_items {
            self.positions
                .entry(color)
                .or_default()
                .extend(new_positions);
        }

        // Deduplicate and remove consumed items for each color
        for (color, positions) in self.positions.iter_mut() {
            // Remove duplicates manually
            let mut unique_positions: Vec<Position> = Vec::new();
            for &pos in positions.iter() {
                if !unique_positions.contains(&pos) {
                    unique_positions.push(pos);
                }
            }
            *positions = unique_positions;

            // Remove items that have been consumed or opened
            // Only check items within any player's visibility range
            let color_copy = *color; // Copy color for closure
            positions.retain(|pos| {
                let is_visible = all_visibility_bounds
                    .iter()
                    .any(|bounds| bounds.contains(pos));
                if is_visible {
                    // We can see this position (by at least one player), so check if item is still there
                    if let Some(tile) = map.get(pos) {
                        validator(tile, pos, color_copy)
                    } else {
                        true // Keep if we haven't seen this position
                    }
                } else {
                    // Out of all visibility ranges - keep the item (items don't move/disappear when not visible)
                    true
                }
            });
        }
    }

    pub fn get_positions(&self, color: Color) -> Option<&[Position]> {
        self.positions.get(&color).map(|v| v.as_slice())
    }

    pub fn has_color(&self, color: Color) -> bool {
        self.positions
            .get(&color)
            .is_some_and(|positions| !positions.is_empty())
    }

    pub fn closest_to(&self, color: Color, reference: Position) -> Option<Position> {
        self.positions.get(&color).and_then(|positions| {
            positions
                .iter()
                .min_by_key(|pos| reference.distance(pos))
                .copied()
        })
    }

    pub fn colors(&self) -> impl Iterator<Item = &Color> {
        self.positions.keys()
    }
}

impl Default for ColoredItemTracker {
    fn default() -> Self {
        Self::new()
    }
}
