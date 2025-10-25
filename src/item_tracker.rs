use crate::swoq_interface::Tile;
use crate::world_state::{Bounds, Color, Pos};
use std::collections::HashMap;

/// Generic tracker for simple items (non-colored)
#[derive(Clone, Debug)]
pub struct ItemTracker {
    positions: Vec<Pos>,
}

impl ItemTracker {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
        }
    }

    /// Update positions: merge newly seen items, deduplicate, and remove items that are gone
    /// Only validates items within the visibility bounds
    #[tracing::instrument(level = "trace", skip(self, map, validator, visibility_bounds), fields(seen_count = seen_items.len()))]
    pub fn update<F>(
        &mut self,
        seen_items: Vec<Pos>,
        map: &HashMap<Pos, Tile>,
        validator: F,
        visibility_bounds: &Bounds,
    ) where
        F: Fn(&Tile) -> bool,
    {

        // Add newly seen items
        self.positions.extend(seen_items);

        // Remove duplicates manually
        let mut unique_positions: Vec<Pos> = Vec::new();
        for &pos in self.positions.iter() {
            if !unique_positions.contains(&pos) {
                unique_positions.push(pos);
            }
        }
        self.positions = unique_positions;

        // Remove items that have been consumed or destroyed
        // Only check items within visibility range
        self.positions.retain(|pos| {
            if visibility_bounds.contains(pos) {
                // We can see this position, so check if item is still there
                if let Some(tile) = map.get(pos) {
                    validator(tile)
                } else {
                    true // Keep if we haven't seen this position
                }
            } else {
                // Out of visibility range - keep the item (items don't move/disappear when not visible)
                true
            }
        });
    }

    pub fn get_positions(&self) -> &[Pos] {
        &self.positions
    }

    pub fn closest_to(&self, reference: Pos) -> Option<Pos> {
        self.positions
            .iter()
            .min_by_key(|pos| reference.distance(pos))
            .copied()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn clear(&mut self) {
        self.positions.clear();
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
    positions: HashMap<Color, Vec<Pos>>,
}

impl ColoredItemTracker {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Update positions for a specific color: merge newly seen items, deduplicate, and remove items that are gone
    /// Only validates items within the visibility bounds
    #[tracing::instrument(level = "trace", skip(self, map, validator, visibility_bounds))]
    pub fn update<F>(
        &mut self,
        seen_items: HashMap<Color, Vec<Pos>>,
        map: &HashMap<Pos, Tile>,
        validator: F,
        visibility_bounds: &Bounds,
    ) where
        F: Fn(&Tile) -> bool,
    {

        // Merge newly seen items with previously known ones
        for (color, new_positions) in seen_items {
            self.positions
                .entry(color)
                .or_default()
                .extend(new_positions);
        }

        // Deduplicate and remove consumed items for each color
        for positions in self.positions.values_mut() {
            // Remove duplicates manually
            let mut unique_positions: Vec<Pos> = Vec::new();
            for &pos in positions.iter() {
                if !unique_positions.contains(&pos) {
                    unique_positions.push(pos);
                }
            }
            *positions = unique_positions;

            // Remove items that have been consumed or opened
            // Only check items within visibility range
            positions.retain(|pos| {
                if visibility_bounds.contains(pos) {
                    // We can see this position, so check if item is still there
                    if let Some(tile) = map.get(pos) {
                        validator(tile)
                    } else {
                        true // Keep if we haven't seen this position
                    }
                } else {
                    // Out of visibility range - keep the item (items don't move/disappear when not visible)
                    true
                }
            });
        }
    }

    pub fn get_positions(&self, color: Color) -> Option<&[Pos]> {
        self.positions.get(&color).map(|v| v.as_slice())
    }

    pub fn has_color(&self, color: Color) -> bool {
        self.positions
            .get(&color)
            .is_some_and(|positions| !positions.is_empty())
    }

    pub fn closest_to(&self, color: Color, reference: Pos) -> Option<Pos> {
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

    pub fn clear(&mut self) {
        self.positions.clear();
    }
}

impl Default for ColoredItemTracker {
    fn default() -> Self {
        Self::new()
    }
}
