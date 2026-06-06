# src/problems/circle_packing.rs

## Role In The System

This module builds factor graphs for circle packing. It owns the circle
domain structs, the intersection and kiss factor constructors, the direct and
fast builders, and helpers for reading circles and measuring overlap.

The conceptual background is in
[`notes/concepts/circle_packing.md`](../../concepts/circle_packing.md).

## State At A Glance

`CirclePackingVariables` stores the variable handles and pairwise intersection
factor handles created for a packing problem. The fast builder also creates a
private `DynamicIntersectionManager`, which tracks which pair factors are
currently enabled.

## Code Walkthrough

### Module Documentation

```rust
//! Circle-packing problem builder.
```

### Imports

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::{
    FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue, create_in_range_factor,
};
```

The fast builder registers graph callbacks that must share mutable manager
state, so it uses `Rc<RefCell<_>>`. `HashMap` stores grid cells for nearby-pair
detection. `StdRng` gives deterministic circle generation.

### Public Data Types

```rust
/// Closed coordinate range used for a packing dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateRange {
    pub lower: f64,
    pub upper: f64,
}

/// A circle represented by its center and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

/// Number of circles to generate for a radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusCount {
    pub radius: f64,
    pub count: usize,
}

/// A fixed circle that packed circles should kiss.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KissingCircle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}
```

These structs are plain data. The graph builder validates ranges and radii
before attaching factors.

```rust
/// Variable and factor handles produced by a circle-packing builder.
#[derive(Debug, Clone, PartialEq)]
pub struct CirclePackingVariables {
    pub x: Vec<VariableNode>,
    pub y: Vec<VariableNode>,
    pub radii: Vec<f64>,
    pub intersection_factors: Vec<Vec<FactorNode>>,
}
```

The `x`, `y`, and `radii` vectors share indexes: circle `i` uses `x[i]`,
`y[i]`, and `radii[i]`. `intersection_factors` is triangular: row `i` stores
pairs `(i, i + 1)`, `(i, i + 2)`, and so on.

### `generate_circles`

```rust
pub fn generate_circles(
    random_seed: u64,
    radii: &[RadiusCount],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
) -> Vec<Circle> {
    validate_range(horizontal_range, "generate_circles horizontal_range");
    validate_range(vertical_range, "generate_circles vertical_range");

    let mut rng = StdRng::seed_from_u64(random_seed);
    let count = radii.iter().map(|radius_count| radius_count.count).sum();
    let mut circles = Vec::with_capacity(count);

    for &radius_count in radii {
        validate_radius(radius_count.radius, "generate_circles radius");
        for _ in 0..radius_count.count {
            circles.push(Circle {
                x: rng.random_range(horizontal_range.lower..=horizontal_range.upper),
                y: rng.random_range(vertical_range.lower..=vertical_range.upper),
                radius: radius_count.radius,
            });
        }
    }

    circles
}
```

The generator preserves radius group order while sampling centers uniformly
inside the coordinate ranges. It reserves exact capacity up front.

### `create_kiss_factor`

```rust
pub fn create_kiss_factor(
    graph: &mut FactorGraph,
    x: VariableNode,
    y: VariableNode,
    center_x: f64,
    center_y: f64,
    exact_distance: f64,
) -> FactorNode {
    assert!(
        exact_distance >= 0.0,
        "create_kiss_factor requires exact_distance >= 0"
    );

    let x_edge = graph.create_edge(x);
    let y_edge = graph.create_edge(y);

    graph.create_factor(
        &[x_edge, y_edge],
        Box::new(move |exchanges, _| {
            let x = exchanges[0].get().value;
            let y = exchanges[1].get().value;
            let dx = x - center_x;
            let dy = y - center_y;
            let distance = dx.hypot(dy);
            let (unit_x, unit_y) = if distance == 0.0 {
                (1.0, 0.0)
            } else {
                (dx / distance, dy / distance)
            };

            exchanges[0].set(WeightedValue::new(
                center_x + exact_distance * unit_x,
                MessageWeight::Standard,
            ));
            exchanges[1].set(WeightedValue::new(
                center_y + exact_distance * unit_y,
                MessageWeight::Standard,
            ));
        }),
    )
}
```

The minimizer projects the incoming center onto the circle centered at
`(center_x, center_y)` with radius `exact_distance`. It emits standard-weight
messages because it actively proposes the projected coordinates.

### `create_intersection_factor`

```rust
pub fn create_intersection_factor(
    graph: &mut FactorGraph,
    x1: VariableNode,
    y1: VariableNode,
    x2: VariableNode,
    y2: VariableNode,
    sum_radius: f64,
) -> FactorNode {
    assert!(
        sum_radius >= 0.0,
        "create_intersection_factor requires sum_radius >= 0"
    );

    let x1_edge = graph.create_edge(x1);
    let y1_edge = graph.create_edge(y1);
    let x2_edge = graph.create_edge(x2);
    let y2_edge = graph.create_edge(y2);

    graph.create_factor(
        &[x1_edge, y1_edge, x2_edge, y2_edge],
        Box::new(move |exchanges, _| {
            let x1 = exchanges[0].get().value;
            let y1 = exchanges[1].get().value;
            let x2 = exchanges[2].get().value;
            let y2 = exchanges[3].get().value;

            let dx = x2 - x1;
            let dy = y2 - y1;
            let distance = dx.hypot(dy);
            let overlap = sum_radius - distance;

            if overlap < 0.0 {
                for exchange in exchanges {
                    let incoming = exchange.get();
                    exchange.set(WeightedValue::new(incoming.value, MessageWeight::Zero));
                }
                return;
            }

            let (unit_x, unit_y) = if distance == 0.0 {
                (1.0, 0.0)
            } else {
                (dx / distance, dy / distance)
            };
            let move_x = 0.5 * overlap * unit_x;
            let move_y = 0.5 * overlap * unit_y;

            exchanges[0].set(WeightedValue::new(x1 - move_x, MessageWeight::Standard));
            exchanges[1].set(WeightedValue::new(y1 - move_y, MessageWeight::Standard));
            exchanges[2].set(WeightedValue::new(x2 + move_x, MessageWeight::Standard));
            exchanges[3].set(WeightedValue::new(y2 + move_y, MessageWeight::Standard));
        }),
    )
}
```

The four-edge factor has no opinion when the circles are separated. When they
overlap, each center moves half the overlap distance in opposite directions.

### Direct Builder

```rust
pub fn add_circle_packing_to_factor_graph(
    graph: &mut FactorGraph,
    circles: &[Circle],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    kissing_circle: Option<KissingCircle>,
) -> CirclePackingVariables {
    validate_range(
        horizontal_range,
        "add_circle_packing_to_factor_graph horizontal_range",
    );
    validate_range(
        vertical_range,
        "add_circle_packing_to_factor_graph vertical_range",
    );
    if let Some(kissing_circle) = kissing_circle {
        validate_radius(
            kissing_circle.radius,
            "add_circle_packing_to_factor_graph kissing radius",
        );
    }
```

Input validation happens before graph mutation where possible.

```rust
    let mut variables = CirclePackingVariables {
        x: Vec::with_capacity(circles.len()),
        y: Vec::with_capacity(circles.len()),
        radii: Vec::with_capacity(circles.len()),
        intersection_factors: Vec::with_capacity(circles.len().saturating_sub(1)),
    };

    for &circle in circles {
        validate_fits(
            circle.radius,
            horizontal_range,
            "add_circle_packing_to_factor_graph horizontal radius",
        );
        validate_fits(
            circle.radius,
            vertical_range,
            "add_circle_packing_to_factor_graph vertical radius",
        );

        let x = graph.create_variable(circle.x, MessageWeight::Standard);
        let y = graph.create_variable(circle.y, MessageWeight::Standard);
        create_in_range_factor(
            graph,
            x,
            horizontal_range.lower + circle.radius,
            horizontal_range.upper - circle.radius,
        );
        create_in_range_factor(
            graph,
            y,
            vertical_range.lower + circle.radius,
            vertical_range.upper - circle.radius,
        );

        variables.x.push(x);
        variables.y.push(y);
        variables.radii.push(circle.radius);
    }
```

Each circle gets two variables and two boundary factors. The radius shrinks the
legal center range so the whole circle remains inside the coordinate range.

```rust
    for first in 0..circles.len().saturating_sub(1) {
        let mut row = Vec::with_capacity(circles.len() - first - 1);
        for second in first + 1..circles.len() {
            row.push(create_intersection_factor(
                graph,
                variables.x[first],
                variables.y[first],
                variables.x[second],
                variables.y[second],
                variables.radii[first] + variables.radii[second],
            ));
        }
        variables.intersection_factors.push(row);
    }
```

The direct builder enables every pairwise intersection factor.

```rust
    if let Some(kissing_circle) = kissing_circle {
        for index in 0..circles.len() {
            create_kiss_factor(
                graph,
                variables.x[index],
                variables.y[index],
                kissing_circle.x,
                kissing_circle.y,
                kissing_circle.radius + variables.radii[index],
            );
        }
    }

    variables
}
```

Optional kiss factors are added after the boundary and intersection factors.

### Fast Builder

```rust
pub fn add_circle_packing_to_factor_graph_fast(
    graph: &mut FactorGraph,
    circles: &[Circle],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    nearby_radius_scale: f64,
) -> CirclePackingVariables {
    let variables =
        add_circle_packing_to_factor_graph(graph, circles, horizontal_range, vertical_range, None);
    let manager = Rc::new(RefCell::new(DynamicIntersectionManager::new(
        &variables,
        horizontal_range,
        vertical_range,
        nearby_radius_scale,
    )));

    manager.borrow_mut().reinitialize(graph);

    let iteration_manager = Rc::clone(&manager);
    graph.add_iteration_graph_callback(Box::new(move |graph| {
        iteration_manager.borrow_mut().update(graph);
    }));

    let reinitialize_manager = Rc::clone(&manager);
    graph.add_reinitialize_graph_callback(Box::new(move |graph| {
        reinitialize_manager.borrow_mut().reinitialize(graph);
    }));

    variables
}
```

The fast builder reuses the direct builder, then immediately disables distant
intersection factors. The graph-aware callbacks keep factor enablement current
after iteration and reinitialization.

### Extraction

```rust
pub fn extract_circles(graph: &FactorGraph, variables: &CirclePackingVariables) -> Vec<Circle> {
    assert!(
        variables.x.len() == variables.y.len() && variables.x.len() == variables.radii.len(),
        "extract_circles requires matching x, y, and radii lengths"
    );

    let mut circles = Vec::with_capacity(variables.radii.len());
    for index in 0..variables.radii.len() {
        circles.push(Circle {
            x: graph.value(variables.x[index]),
            y: graph.value(variables.y[index]),
            radius: variables.radii[index],
        });
    }
    circles
}
```

Extraction turns graph values back into domain structs without changing the
graph.

### `max_overlap`

```rust
pub fn max_overlap(
    graph: &FactorGraph,
    variables: &CirclePackingVariables,
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
) -> f64 {
    validate_range(horizontal_range, "max_overlap horizontal_range");
    validate_range(vertical_range, "max_overlap vertical_range");

    let circles = extract_circles(graph, variables);
    let mut max_overlap: f64 = 0.0;

    for circle in &circles {
        max_overlap = max_overlap.max(horizontal_range.lower - (circle.x - circle.radius));
        max_overlap = max_overlap.max((circle.x + circle.radius) - horizontal_range.upper);
        max_overlap = max_overlap.max(vertical_range.lower - (circle.y - circle.radius));
        max_overlap = max_overlap.max((circle.y + circle.radius) - vertical_range.upper);
    }

    for first in 0..circles.len().saturating_sub(1) {
        for second in first + 1..circles.len() {
            let dx = circles[first].x - circles[second].x;
            let dy = circles[first].y - circles[second].y;
            let overlap = circles[first].radius + circles[second].radius - dx.hypot(dy);
            max_overlap = max_overlap.max(overlap);
        }
    }

    max_overlap
}
```

The overlap metric reports the largest current violation. Negative values are
ignored by starting at `0.0`, so a valid packing reports exactly `0.0`.
Callers can combine this with `FactorGraph::iterate_until_satisfied()` when
they want convergence to mean both stable variable beliefs and satisfied
packing constraints.

### Dynamic Grid Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridCell {
    x: isize,
    y: isize,
}

struct DynamicIntersectionManager {
    variables: CirclePackingVariables,
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    active_pairs: Vec<bool>,
    search_buffer: f64,
    cell_size: f64,
}
```

`GridCell` is hashable so it can key a `HashMap`. The manager stores a clone
of the handle collection, not graph data.

### `DynamicIntersectionManager::new`

```rust
impl DynamicIntersectionManager {
    fn new(
        variables: &CirclePackingVariables,
        horizontal_range: CoordinateRange,
        vertical_range: CoordinateRange,
        nearby_radius_scale: f64,
    ) -> Self {
        assert!(
            nearby_radius_scale >= 0.0,
            "add_circle_packing_to_factor_graph_fast requires nearby_radius_scale >= 0"
        );

        let max_radius = variables.radii.iter().copied().fold(0.0, f64::max);
        let search_buffer = nearby_radius_scale * max_radius;
        let cell_size = (max_radius + search_buffer).max(1e-12);

        Self {
            variables: variables.clone(),
            horizontal_range,
            vertical_range,
            active_pairs: vec![false; num_pairs(variables.radii.len())],
            search_buffer,
            cell_size,
        }
    }
```

The cell size is based on the largest radius plus a search buffer. The small
floor prevents a zero cell size when all radii are zero.

### Dynamic Updates

```rust
    fn reinitialize(&mut self, graph: &mut FactorGraph) {
        self.disable_all(graph);
        self.update(graph);
    }

    fn update(&mut self, graph: &mut FactorGraph) {
        let circles = extract_circles(graph, &self.variables);
        let mut should_enable = vec![false; num_pairs(circles.len())];
        let mut grid: HashMap<GridCell, Vec<usize>> = HashMap::new();

        for (index, circle) in circles.iter().enumerate() {
            grid.entry(self.grid_cell(*circle)).or_default().push(index);
        }
```

Reinitialization starts from all pairs disabled and then applies the same
nearby-pair scan as a normal update.

```rust
        for (&cell, indexes) in &grid {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let neighbor = GridCell {
                        x: cell.x + dx,
                        y: cell.y + dy,
                    };
                    let Some(neighbor_indexes) = grid.get(&neighbor) else {
                        continue;
                    };

                    for &first in indexes {
                        for &second in neighbor_indexes {
                            if first >= second {
                                continue;
                            }

                            let circle_1 = circles[first];
                            let circle_2 = circles[second];
                            let nearby_distance =
                                circle_1.radius.max(circle_2.radius) + self.search_buffer;

                            if (circle_1.x - circle_2.x).abs() <= nearby_distance
                                && (circle_1.y - circle_2.y).abs() <= nearby_distance
                            {
                                should_enable[pair_index(first, second, circles.len())] = true;
                            }
                        }
                    }
                }
            }
        }
```

Only the current cell and its eight neighbors need inspection. The rectangular
distance check is conservative: it can enable extra factors, but it should not
miss nearby circles.

```rust
        for first in 0..circles.len().saturating_sub(1) {
            for second in first + 1..circles.len() {
                let pair = pair_index(first, second, circles.len());
                if should_enable[pair] == self.active_pairs[pair] {
                    continue;
                }

                graph.set_factor_enabled(
                    factor_for_pair(&self.variables, first, second),
                    should_enable[pair],
                );
                self.active_pairs[pair] = should_enable[pair];
            }
        }
    }
```

The graph is mutated only when a pair changes state. This avoids resetting
edges unnecessarily.

```rust
    fn disable_all(&mut self, graph: &mut FactorGraph) {
        for first in 0..self.variables.radii.len().saturating_sub(1) {
            for second in first + 1..self.variables.radii.len() {
                let pair = pair_index(first, second, self.variables.radii.len());
                graph.set_factor_enabled(factor_for_pair(&self.variables, first, second), false);
                self.active_pairs[pair] = false;
            }
        }
    }

    fn grid_cell(&self, circle: Circle) -> GridCell {
        GridCell {
            x: ((circle.x - self.horizontal_range.lower) / self.cell_size).floor() as isize,
            y: ((circle.y - self.vertical_range.lower) / self.cell_size).floor() as isize,
        }
    }
}
```

`grid_cell` anchors the grid at the lower-left coordinate range corner.

### Pair Helpers

```rust
fn num_pairs(count: usize) -> usize {
    count.saturating_sub(1) * count / 2
}

fn pair_index(first: usize, second: usize, count: usize) -> usize {
    debug_assert!(first < second);
    first * (2 * count - first - 1) / 2 + (second - first - 1)
}

fn factor_for_pair(variables: &CirclePackingVariables, first: usize, second: usize) -> FactorNode {
    variables.intersection_factors[first][second - first - 1]
}
```

These helpers flatten unordered circle pairs into indexes in `active_pairs`
and recover the matching triangular factor handle.

### Validation Helpers

```rust
fn validate_range(range: CoordinateRange, label: &str) {
    assert!(
        range.upper >= range.lower,
        "{label} requires upper >= lower"
    );
}

fn validate_radius(radius: f64, label: &str) {
    assert!(radius >= 0.0, "{label} requires radius >= 0");
}

fn validate_fits(radius: f64, range: CoordinateRange, label: &str) {
    validate_radius(radius, label);
    assert!(
        range.lower + radius <= range.upper - radius,
        "{label} requires the circle to fit inside the range"
    );
}
```

Invalid geometry is treated as a programmer error and therefore panics.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn unit_range() -> CoordinateRange {
        CoordinateRange {
            lower: 0.0,
            upper: 1.0,
        }
    }

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }
```

The test helpers keep all test fixtures in the unit square unless a test is
specifically checking boundary violations.

```rust
    #[test]
    fn generate_circles_is_deterministic() { /* ... */ }

    #[test]
    fn add_to_factor_graph_counts() { /* ... */ }

    #[test]
    fn fast_builder_enables_only_nearby_intersections() { /* ... */ }

    #[test]
    fn fast_builder_enables_intersections_after_iteration() { /* ... */ }

    #[test]
    fn max_overlap_reports_boundary_and_intersection_violations() { /* ... */ }

    #[test]
    fn boundary_constraints_move_circle_inside() { /* ... */ }

    #[test]
    fn intersection_factor_separates_overlapping_circles() { /* ... */ }

    #[test]
    fn intersection_factor_handles_coincident_centers() { /* ... */ }

    #[test]
    fn kiss_factor_handles_coincident_center() { /* ... */ }

    #[test]
    fn non_fast_generated_packing_converges() {
        let delta = 1e-5;
        let mut graph = FactorGraph::new(0.07, delta, 0);
        let circles = generate_circles(
            777,
            &[RadiusCount {
                radius: 0.055,
                count: 12,
            }],
            unit_range(),
            unit_range(),
        );
        let variables = add_circle_packing_to_factor_graph(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            None,
        );

        let converged = graph.iterate_until_satisfied(2000, |graph| {
            max_overlap(graph, &variables, unit_range(), unit_range()) < 100.0 * delta
        });

        assert!(
            converged,
            "graph did not converge; max overlap was {}, max message difference was {:?}",
            max_overlap(&graph, &variables, unit_range(), unit_range()),
            graph.max_message_difference()
        );
        assert!(
            max_overlap(&graph, &variables, unit_range(), unit_range()) < 100.0 * delta,
            "max overlap was {}",
            max_overlap(&graph, &variables, unit_range(), unit_range())
        );
    }

    #[test]
    #[should_panic(expected = "radius >= 0")]
    fn generate_circles_rejects_negative_radius() { /* ... */ }

    #[test]
    #[should_panic(expected = "fit inside the range")]
    fn add_to_factor_graph_rejects_circles_that_do_not_fit() { /* ... */ }

    #[test]
    #[should_panic(expected = "radius >= 0")]
    fn add_to_factor_graph_rejects_negative_kissing_radius() { /* ... */ }
}
```

The tests cover deterministic generation, graph shape, dynamic factor
enablement, boundary projection, pair projection, kiss projection, convergence
on a generated packing using `max_overlap` as the domain satisfaction predicate,
and validation panics.

## Important Invariants

- `CirclePackingVariables.x`, `y`, and `radii` must remain the same length.
- `intersection_factors` uses triangular storage, so pair-index helpers must
  stay consistent with builder insertion order.
- Disabled intersection factors must be re-enabled with current variable
  values, which is handled by `FactorGraph::set_factor_enabled`.
- The fast builder must never remove factors; it only toggles factor state.

## Algorithm Context

Circle packing expresses geometric feasibility as message-passing constraints:
boundary factors clamp coordinates, intersection factors project pairs apart,
and kiss factors project centers to a fixed distance. The solver then combines
all factor proposals through the usual TWA variable consensus.

## Extension Notes

If future builders add different spatial indexing, keep the graph callback
boundary clear: callbacks may toggle factors, but they should not create new
variables, edges, or factors after iteration begins.
