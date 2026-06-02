//! Circle-packing problem builder.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::{
    FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue, create_in_range_factor,
};

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

/// Variable and factor handles produced by a circle-packing builder.
#[derive(Debug, Clone, PartialEq)]
pub struct CirclePackingVariables {
    pub x: Vec<VariableNode>,
    pub y: Vec<VariableNode>,
    pub radii: Vec<f64>,
    pub intersection_factors: Vec<Vec<FactorNode>>,
}

/// Generates random circle centers for the requested radius counts.
///
/// Each radius group is emitted in input order. Centers are sampled uniformly
/// across the provided coordinate ranges.
///
/// # Panics
///
/// Panics if a range is inverted or a radius is negative.
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

/// Creates a factor that keeps a circle center at an exact distance from a fixed point.
///
/// # Panics
///
/// Panics if `exact_distance` is negative.
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

/// Creates a factor that separates two circles when their centers are too close.
///
/// # Panics
///
/// Panics if `sum_radius` is negative.
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

/// Adds a complete circle-packing problem to a graph.
///
/// # Panics
///
/// Panics if ranges are inverted, a radius is negative, or a circle cannot fit
/// within the supplied ranges.
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

/// Adds a circle-packing problem with dynamically enabled intersection factors.
///
/// Intersection factors are still allocated for every pair, but distant pairs
/// are disabled and re-enabled only when a spatial grid says they are nearby.
///
/// # Panics
///
/// Panics if `nearby_radius_scale` is negative, or for the same inputs rejected
/// by [`add_circle_packing_to_factor_graph`].
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

/// Reads circle centers back out of a factor graph.
///
/// # Panics
///
/// Panics if the variable collection is internally inconsistent or contains
/// invalid handles.
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

/// Returns the largest current boundary or pairwise-circle overlap.
///
/// A result of `0.0` means all circles are inside the ranges and no pair is
/// overlapping.
///
/// # Panics
///
/// Panics if a range is inverted or the variable collection is inconsistent.
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

    #[test]
    fn generate_circles_is_deterministic() {
        let radii = [
            RadiusCount {
                radius: 0.1,
                count: 2,
            },
            RadiusCount {
                radius: 0.2,
                count: 1,
            },
        ];

        let first = generate_circles(123, &radii, unit_range(), unit_range());
        let second = generate_circles(123, &radii, unit_range(), unit_range());
        let different = generate_circles(124, &radii, unit_range(), unit_range());

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].radius, 0.1);
        assert_eq!(first[1].radius, 0.1);
        assert_eq!(first[2].radius, 0.2);
        assert!(
            first.iter().all(|circle| {
                (0.0..=1.0).contains(&circle.x) && (0.0..=1.0).contains(&circle.y)
            })
        );
    }

    #[test]
    fn add_to_factor_graph_counts() {
        let circles = [
            Circle {
                x: 0.2,
                y: 0.2,
                radius: 0.1,
            },
            Circle {
                x: 0.4,
                y: 0.2,
                radius: 0.1,
            },
            Circle {
                x: 0.8,
                y: 0.8,
                radius: 0.1,
            },
        ];
        let mut graph = FactorGraph::default();

        let variables = add_circle_packing_to_factor_graph(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            None,
        );

        assert_eq!(graph.num_variables(), 6);
        assert_eq!(graph.num_factors(), 9);
        assert_eq!(graph.num_edges(), 18);
        assert_eq!(variables.intersection_factors.len(), 2);
        assert_eq!(variables.intersection_factors[0].len(), 2);
        assert_eq!(variables.intersection_factors[1].len(), 1);
    }

    #[test]
    fn fast_builder_enables_only_nearby_intersections() {
        let circles = [
            Circle {
                x: 0.2,
                y: 0.2,
                radius: 0.1,
            },
            Circle {
                x: 0.42,
                y: 0.2,
                radius: 0.1,
            },
            Circle {
                x: 0.8,
                y: 0.8,
                radius: 0.1,
            },
        ];
        let mut graph = FactorGraph::default();

        let variables = add_circle_packing_to_factor_graph_fast(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            1.4,
        );

        assert_eq!(graph.num_variables(), 6);
        assert_eq!(graph.num_factors(), 9);
        assert_eq!(graph.num_edges(), 18);
        assert_eq!(graph.num_enabled_factors(), 7);
        assert_eq!(graph.num_enabled_edges(), 10);
        assert!(graph.is_factor_enabled(variables.intersection_factors[0][0]));
        assert!(!graph.is_factor_enabled(variables.intersection_factors[0][1]));
        assert!(!graph.is_factor_enabled(variables.intersection_factors[1][0]));
    }

    #[test]
    fn fast_builder_enables_intersections_after_iteration() {
        let circles = [
            Circle {
                x: 0.75,
                y: 0.5,
                radius: 0.1,
            },
            Circle {
                x: 1.2,
                y: 0.5,
                radius: 0.1,
            },
        ];
        let mut graph = FactorGraph::default();

        let variables = add_circle_packing_to_factor_graph_fast(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            1.4,
        );

        assert!(!graph.is_factor_enabled(variables.intersection_factors[0][0]));
        assert_eq!(graph.num_enabled_factors(), 4);

        graph.iterate();

        assert!(graph.is_factor_enabled(variables.intersection_factors[0][0]));
        assert_eq!(graph.num_enabled_factors(), 5);
    }

    #[test]
    fn max_overlap_reports_boundary_and_intersection_violations() {
        let circles = [
            Circle {
                x: -0.05,
                y: 0.5,
                radius: 0.1,
            },
            Circle {
                x: 0.12,
                y: 0.5,
                radius: 0.1,
            },
        ];
        let mut graph = FactorGraph::default();
        let variables = add_circle_packing_to_factor_graph(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            None,
        );

        assert!(nearly_equal(
            max_overlap(&graph, &variables, unit_range(), unit_range()),
            0.15
        ));
    }

    #[test]
    fn boundary_constraints_move_circle_inside() {
        let circles = [Circle {
            x: 0.0,
            y: 1.0,
            radius: 0.1,
        }];
        let mut graph = FactorGraph::default();
        let variables = add_circle_packing_to_factor_graph(
            &mut graph,
            &circles,
            unit_range(),
            unit_range(),
            None,
        );

        assert!(graph.iterate_until_converged(100));
        let packed = extract_circles(&graph, &variables);
        assert!(nearly_equal(packed[0].x, 0.1));
        assert!(nearly_equal(packed[0].y, 0.9));
        assert!(nearly_equal(
            max_overlap(&graph, &variables, unit_range(), unit_range()),
            0.0
        ));
    }

    #[test]
    fn intersection_factor_separates_overlapping_circles() {
        let mut graph = FactorGraph::default();
        let x1 = graph.create_variable(0.45, MessageWeight::Standard);
        let y1 = graph.create_variable(0.5, MessageWeight::Standard);
        let x2 = graph.create_variable(0.55, MessageWeight::Standard);
        let y2 = graph.create_variable(0.5, MessageWeight::Standard);

        create_intersection_factor(&mut graph, x1, y1, x2, y2, 0.2);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(x1), 0.4));
        assert!(nearly_equal(graph.value(y1), 0.5));
        assert!(nearly_equal(graph.value(x2), 0.6));
        assert!(nearly_equal(graph.value(y2), 0.5));
    }

    #[test]
    fn intersection_factor_handles_coincident_centers() {
        let mut graph = FactorGraph::default();
        let x1 = graph.create_variable(0.5, MessageWeight::Standard);
        let y1 = graph.create_variable(0.5, MessageWeight::Standard);
        let x2 = graph.create_variable(0.5, MessageWeight::Standard);
        let y2 = graph.create_variable(0.5, MessageWeight::Standard);

        create_intersection_factor(&mut graph, x1, y1, x2, y2, 0.2);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(x1), 0.4));
        assert!(nearly_equal(graph.value(y1), 0.5));
        assert!(nearly_equal(graph.value(x2), 0.6));
        assert!(nearly_equal(graph.value(y2), 0.5));
    }

    #[test]
    fn kiss_factor_handles_coincident_center() {
        let mut graph = FactorGraph::default();
        let x = graph.create_variable(0.0, MessageWeight::Standard);
        let y = graph.create_variable(0.0, MessageWeight::Standard);

        create_kiss_factor(&mut graph, x, y, 0.0, 0.0, 1.0);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(x), 1.0));
        assert!(nearly_equal(graph.value(y), 0.0));
    }

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

        graph.iterate_until_converged(2000);

        assert!(
            max_overlap(&graph, &variables, unit_range(), unit_range()) < 100.0 * delta,
            "max overlap was {}",
            max_overlap(&graph, &variables, unit_range(), unit_range())
        );
    }

    #[test]
    #[should_panic(expected = "radius >= 0")]
    fn generate_circles_rejects_negative_radius() {
        generate_circles(
            0,
            &[RadiusCount {
                radius: -0.1,
                count: 1,
            }],
            unit_range(),
            unit_range(),
        );
    }

    #[test]
    #[should_panic(expected = "fit inside the range")]
    fn add_to_factor_graph_rejects_circles_that_do_not_fit() {
        let mut graph = FactorGraph::default();
        add_circle_packing_to_factor_graph(
            &mut graph,
            &[Circle {
                x: 0.5,
                y: 0.5,
                radius: 0.6,
            }],
            unit_range(),
            unit_range(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "radius >= 0")]
    fn add_to_factor_graph_rejects_negative_kissing_radius() {
        let mut graph = FactorGraph::default();
        add_circle_packing_to_factor_graph(
            &mut graph,
            &[Circle {
                x: 0.5,
                y: 0.5,
                radius: 0.1,
            }],
            unit_range(),
            unit_range(),
            Some(KissingCircle {
                x: 0.5,
                y: 0.5,
                radius: -0.1,
            }),
        );
    }
}
