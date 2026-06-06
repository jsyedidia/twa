use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

use crate::edge_data::EdgeData;
use crate::factor_data::FactorData;
use crate::factor_node::FactorNode;
use crate::graph_edge::GraphEdge;
use crate::variable_data::VariableData;
use crate::variable_node::VariableNode;
use crate::weighted_value::{MessageWeight, MinimizationFn, WeightedValue};

type Callback = Box<dyn FnMut()>;
type GraphCallback = Box<dyn FnMut(&mut FactorGraph)>;

/// The main public type of the TWA crate.
///
/// A `FactorGraph` holds variables, edges, and factors, and drives the
/// three-weight ADMM iteration until convergence.
pub struct FactorGraph {
    learning_rate: f64,
    convergence_delta: f64,
    rng: Box<dyn RngCore>,
    iterations: usize,
    converged: bool,
    variables: Vec<VariableData>,
    edges: Vec<EdgeData>,
    factors: Vec<FactorData>,
    iteration_callbacks: Vec<Callback>,
    reinitialize_callbacks: Vec<Callback>,
    iteration_graph_callbacks: Vec<GraphCallback>,
    reinitialize_graph_callbacks: Vec<GraphCallback>,
}

impl FactorGraph {
    /// Creates a new factor graph with the given parameters.
    ///
    /// - `learning_rate`: step size for the disagreement update (alpha in ADMM).
    /// - `convergence_delta`: threshold below which variable belief changes are
    ///   considered converged.
    /// - `random_seed`: seed for the random number generator used by minimizers.
    pub fn new(learning_rate: f64, convergence_delta: f64, random_seed: u64) -> Self {
        Self {
            learning_rate,
            convergence_delta,
            rng: Box::new(StdRng::seed_from_u64(random_seed)),
            iterations: 0,
            converged: false,
            variables: Vec::new(),
            edges: Vec::new(),
            factors: Vec::new(),
            iteration_callbacks: Vec::new(),
            reinitialize_callbacks: Vec::new(),
            iteration_graph_callbacks: Vec::new(),
            reinitialize_graph_callbacks: Vec::new(),
        }
    }

    /// Returns the current learning rate.
    pub fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    /// Sets the learning rate.
    pub fn set_learning_rate(&mut self, learning_rate: f64) {
        self.learning_rate = learning_rate;
    }

    /// Returns the convergence delta threshold.
    pub fn convergence_delta(&self) -> f64 {
        self.convergence_delta
    }

    /// Sets the convergence delta threshold.
    pub fn set_convergence_delta(&mut self, convergence_delta: f64) {
        self.convergence_delta = convergence_delta;
    }

    /// Reseeds the random number generator.
    pub fn set_random_seed(&mut self, random_seed: u64) {
        self.rng = Box::new(StdRng::seed_from_u64(random_seed));
    }

    /// Returns the number of iterations performed since the last reinitialize.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Returns whether the graph has converged.
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// Returns the total number of variables.
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    /// Returns the total number of edges.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Returns the number of currently enabled edges.
    pub fn num_enabled_edges(&self) -> usize {
        self.edges.iter().filter(|e| e.is_enabled()).count()
    }

    /// Returns the total number of factors.
    pub fn num_factors(&self) -> usize {
        self.factors.len()
    }

    /// Returns the number of currently enabled factors.
    pub fn num_enabled_factors(&self) -> usize {
        self.factors.iter().filter(|f| f.is_enabled()).count()
    }

    /// Returns the largest available enabled-edge message difference.
    ///
    /// Message differences are a dual-state diagnostic. They are not the
    /// default convergence criterion; `iterate()` converges when variable
    /// beliefs stop changing. Returns `None` until every enabled edge has
    /// enough history to compute a message difference.
    pub fn max_message_difference(&self) -> Option<f64> {
        let mut max_difference: f64 = 0.0;

        for edge in &self.edges {
            if !edge.is_enabled() {
                continue;
            }
            let difference = edge.message_difference()?;
            max_difference = max_difference.max(difference);
        }

        Some(max_difference)
    }

    /// Creates a new variable with the given initial value and weight.
    ///
    /// # Panics
    ///
    /// Does not panic; always succeeds.
    pub fn create_variable(
        &mut self,
        initial_value: f64,
        initial_weight: MessageWeight,
    ) -> VariableNode {
        let node = VariableNode::new(self.variables.len());
        self.variables.push(VariableData::new(WeightedValue::new(
            initial_value,
            initial_weight,
        )));
        node
    }

    /// Creates a new edge connecting to the given variable.
    ///
    /// # Panics
    ///
    /// Panics if the variable handle is invalid or out of bounds.
    pub fn create_edge(&mut self, variable: VariableNode) -> GraphEdge {
        self.validate_variable(variable);

        let edge = GraphEdge::new(self.edges.len());
        let initial = self.variables[variable.index()].initial_value();
        self.edges.push(EdgeData::new(variable, initial));
        self.variables[variable.index()].add_edge(edge);
        edge
    }

    /// Creates a new factor over the given edges with the given minimization function.
    ///
    /// # Panics
    ///
    /// Panics if any edge handle is invalid or out of bounds.
    pub fn create_factor(
        &mut self,
        edges: &[GraphEdge],
        minimization_function: MinimizationFn,
    ) -> FactorNode {
        for &edge in edges {
            self.validate_edge(edge);
        }

        let node = FactorNode::new(self.factors.len());
        self.factors
            .push(FactorData::new(edges, minimization_function));
        node
    }

    /// Returns the current value of a variable.
    ///
    /// # Panics
    ///
    /// Panics if the variable handle is invalid or out of bounds.
    pub fn value(&self, variable: VariableNode) -> f64 {
        self.validate_variable(variable);
        self.variables[variable.index()].value()
    }

    /// Returns the current weight of a variable.
    ///
    /// # Panics
    ///
    /// Panics if the variable handle is invalid or out of bounds.
    pub fn weight(&self, variable: VariableNode) -> MessageWeight {
        self.validate_variable(variable);
        self.variables[variable.index()].weight()
    }

    /// Returns whether a factor is currently enabled.
    ///
    /// # Panics
    ///
    /// Panics if the factor handle is invalid or out of bounds.
    pub fn is_factor_enabled(&self, factor: FactorNode) -> bool {
        self.validate_factor(factor);
        self.factors[factor.index()].is_enabled()
    }

    /// Enables or disables a factor.
    ///
    /// When a factor is disabled, its edges are marked disabled and its
    /// minimization function is not called during iteration. When re-enabled,
    /// edges are reset with the current variable value.
    ///
    /// # Panics
    ///
    /// Panics if the factor handle is invalid or out of bounds.
    pub fn set_factor_enabled(&mut self, factor: FactorNode, enabled: bool) {
        self.validate_factor(factor);

        if enabled {
            self.enable_factor(factor.index());
        } else {
            self.disable_factor(factor.index());
        }
    }

    /// Runs one iteration of the TWA algorithm.
    ///
    /// Returns `true` if the graph has converged (all variable belief values
    /// changed by at most `convergence_delta`).
    pub fn iterate(&mut self) -> bool {
        self.iterate_with_satisfaction(&mut |_| true)
    }

    fn iterate_with_satisfaction<F>(&mut self, is_satisfied: &mut F) -> bool
    where
        F: FnMut(&FactorGraph) -> bool,
    {
        if self.converged {
            return true;
        }

        // Factor pass: each factor reads messages, calls its minimizer,
        // writes results back.
        for factor in &mut self.factors {
            factor.minimize(&mut self.edges, self.rng.as_mut());
        }

        // Variable pass: compute consensus, update edges.
        let mut beliefs_converged = true;
        for variable in &mut self.variables {
            let (result, has_lone_standard) = {
                let enabled_edges = variable.enabled_edges(&self.edges);
                (
                    Self::enforce_variable_equality(&self.edges, enabled_edges),
                    Self::has_lone_standard_message_to_variable(&self.edges, enabled_edges),
                )
            };
            beliefs_converged &=
                Self::belief_converged(variable.value(), result.value, self.convergence_delta);
            variable.update_result(result);

            for &edge in variable.enabled_edges(&self.edges) {
                let reset_disagreement = has_lone_standard
                    && self.edges[edge.index()]
                        .weighted_message_to_variable()
                        .weight
                        == MessageWeight::Standard;
                self.edges[edge.index()].set_result_from_variable(
                    result,
                    self.learning_rate,
                    reset_disagreement,
                );
            }
        }

        self.iterations += 1;
        self.converged = beliefs_converged;

        for callback in &mut self.iteration_callbacks {
            callback();
        }
        self.run_iteration_graph_callbacks();

        if self.converged && !is_satisfied(self) {
            self.converged = false;
        }

        self.converged
    }

    /// Runs up to `max_iterations` iterations, stopping early if convergence
    /// is reached.
    ///
    /// Returns `true` if the graph has converged.
    pub fn iterate_until_converged(&mut self, max_iterations: usize) -> bool {
        for _ in 0..max_iterations {
            if self.iterate() {
                return true;
            }
        }
        self.converged
    }

    /// Runs up to `max_iterations` iterations, stopping early if variable
    /// beliefs converge and `is_satisfied` returns `true`.
    ///
    /// Use this for domain-specific stopping conditions. For example, circle
    /// packing can require `max_overlap(...) <= tolerance` in addition to
    /// stable variable beliefs.
    pub fn iterate_until_satisfied<F>(&mut self, max_iterations: usize, mut is_satisfied: F) -> bool
    where
        F: FnMut(&FactorGraph) -> bool,
    {
        if self.converged {
            if is_satisfied(self) {
                return true;
            }
            self.converged = false;
        }

        for _ in 0..max_iterations {
            if self.iterate_with_satisfaction(&mut is_satisfied) {
                return true;
            }
        }
        self.converged
    }

    /// Resets the graph to its initial state: all variables and edges are
    /// restored, iteration count is cleared, and reinitialize callbacks are
    /// invoked.
    pub fn reinitialize(&mut self) {
        for variable in &mut self.variables {
            let initial = variable.initial_value();
            for &edge in variable.edges() {
                self.edges[edge.index()].reset(initial);
            }
            variable.reset();
        }

        for factor in &mut self.factors {
            factor.reset();
        }

        self.iterations = 0;
        self.converged = false;

        for callback in &mut self.reinitialize_callbacks {
            callback();
        }
        self.run_reinitialize_graph_callbacks();
    }

    /// Registers a callback invoked after each iteration.
    pub fn add_iteration_callback(&mut self, callback: Box<dyn FnMut()>) {
        self.iteration_callbacks.push(callback);
    }

    /// Registers a callback invoked after each reinitialization.
    pub fn add_reinitialize_callback(&mut self, callback: Box<dyn FnMut()>) {
        self.reinitialize_callbacks.push(callback);
    }

    /// Registers a callback invoked after each iteration with mutable graph access.
    pub fn add_iteration_graph_callback(&mut self, callback: Box<dyn FnMut(&mut FactorGraph)>) {
        self.iteration_graph_callbacks.push(callback);
    }

    /// Registers a callback invoked after each reinitialization with mutable graph access.
    pub fn add_reinitialize_graph_callback(&mut self, callback: Box<dyn FnMut(&mut FactorGraph)>) {
        self.reinitialize_graph_callbacks.push(callback);
    }

    // --- Private helpers ---

    fn run_iteration_graph_callbacks(&mut self) {
        let mut callbacks = std::mem::take(&mut self.iteration_graph_callbacks);
        for callback in &mut callbacks {
            callback(self);
        }
        self.iteration_graph_callbacks = callbacks;
    }

    fn run_reinitialize_graph_callbacks(&mut self) {
        let mut callbacks = std::mem::take(&mut self.reinitialize_graph_callbacks);
        for callback in &mut callbacks {
            callback(self);
        }
        self.reinitialize_graph_callbacks = callbacks;
    }

    fn validate_variable(&self, variable: VariableNode) {
        assert!(
            variable.is_valid() && variable.index() < self.variables.len(),
            "FactorGraph references an invalid variable"
        );
    }

    fn validate_edge(&self, edge: GraphEdge) {
        assert!(
            edge.is_valid() && edge.index() < self.edges.len(),
            "FactorGraph references an invalid edge"
        );
    }

    fn validate_factor(&self, factor: FactorNode) {
        assert!(
            factor.is_valid() && factor.index() < self.factors.len(),
            "FactorGraph references an invalid factor"
        );
    }

    fn enable_factor(&mut self, factor_index: usize) {
        if !self.factors[factor_index].enable() {
            return;
        }

        for exchange in self.factors[factor_index].exchanges().to_vec() {
            let edge = exchange.edge();
            let variable = self.edges[edge.index()].variable();
            let value = self.variables[variable.index()].value();
            self.edges[edge.index()].reset(WeightedValue::new(value, MessageWeight::Standard));
            self.variables[variable.index()].reenable_edge(edge);
        }

        self.converged = false;
    }

    fn disable_factor(&mut self, factor_index: usize) {
        if !self.factors[factor_index].disable() {
            return;
        }

        for exchange in self.factors[factor_index].exchanges().to_vec() {
            let edge = exchange.edge();
            let variable = self.edges[edge.index()].variable();
            self.edges[edge.index()].disable();
            self.variables[variable.index()].force_enabled_edges_update();
        }

        self.converged = false;
    }

    fn enforce_variable_equality(edges: &[EdgeData], enabled_edges: &[GraphEdge]) -> WeightedValue {
        let mut all_sum = 0.0;
        let mut standard_sum = 0.0;
        let mut standard_count: usize = 0;

        for &edge in enabled_edges {
            let message = edges[edge.index()].weighted_message_to_variable();
            if message.weight == MessageWeight::Infinite {
                return message;
            }
            if message.weight == MessageWeight::Standard {
                standard_sum += message.value;
                standard_count += 1;
            }
            all_sum += message.value;
        }

        if standard_count > 0 {
            WeightedValue::new(
                standard_sum / standard_count as f64,
                MessageWeight::Standard,
            )
        } else {
            WeightedValue::new(all_sum / enabled_edges.len() as f64, MessageWeight::Zero)
        }
    }

    fn has_lone_standard_message_to_variable(
        edges: &[EdgeData],
        enabled_edges: &[GraphEdge],
    ) -> bool {
        let mut standard_count: usize = 0;
        for &edge in enabled_edges {
            let weight = edges[edge.index()].weighted_message_to_variable().weight;
            if weight == MessageWeight::Infinite {
                return false;
            }
            if weight == MessageWeight::Standard {
                standard_count += 1;
            }
        }
        standard_count == 1
    }

    fn belief_converged(old_value: f64, new_value: f64, convergence_delta: f64) -> bool {
        (old_value - new_value).abs() <= convergence_delta
    }
}

impl Default for FactorGraph {
    fn default() -> Self {
        Self::new(1.0, 1e-5, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }

    /// Helper: creates a factor that always outputs the given known value
    /// with infinite weight on every exchange.
    fn known_value_minimizer(value: f64) -> MinimizationFn {
        Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
            for exchange in exchanges.iter_mut() {
                exchange.set(WeightedValue::new(value, MessageWeight::Infinite));
            }
        })
    }

    /// Helper: creates a factor that clamps each exchange's value to the given
    /// range with standard weight.
    fn in_range_minimizer(low: f64, high: f64) -> MinimizationFn {
        Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
            for exchange in exchanges.iter_mut() {
                let clamped = exchange.get().value.clamp(low, high);
                exchange.set(WeightedValue::new(clamped, MessageWeight::Standard));
            }
        })
    }

    #[test]
    fn known_value_converges() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(7.0));

        assert!(graph.iterate_until_converged(100));
        assert!(graph.converged());
        assert!(nearly_equal(graph.value(variable), 7.0));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
        assert!(graph.iterations() >= 2);
    }

    #[test]
    fn range_clamps_below_above_and_inside() {
        // Below range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(-2.0, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 0.0));
        }

        // Above range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(5.0, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 1.0));
        }

        // Inside range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(0.4, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 0.4));
        }
    }

    #[test]
    fn reinitialize_resets_graph_state() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(2.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(4.0));

        use std::cell::Cell;
        use std::rc::Rc;

        let iteration_count = Rc::new(Cell::new(0));
        let reinitialize_count = Rc::new(Cell::new(0));

        let ic = Rc::clone(&iteration_count);
        graph.add_iteration_callback(Box::new(move || {
            ic.set(ic.get() + 1);
        }));

        let rc = Rc::clone(&reinitialize_count);
        graph.add_reinitialize_callback(Box::new(move || {
            rc.set(rc.get() + 1);
        }));

        assert!(graph.iterate_until_converged(100));
        assert!(graph.converged());
        assert!(graph.iterations() > 0);
        assert_eq!(iteration_count.get(), graph.iterations());
        assert!(nearly_equal(graph.value(variable), 4.0));

        graph.reinitialize();
        assert!(!graph.converged());
        assert_eq!(graph.iterations(), 0);
        assert_eq!(reinitialize_count.get(), 1);
        assert!(nearly_equal(graph.value(variable), 2.0));

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 4.0));
    }

    #[test]
    fn factor_enable_disable() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(1.0, MessageWeight::Standard);

        let edge1 = graph.create_edge(variable);
        graph.create_factor(&[edge1], in_range_minimizer(0.0, 20.0));

        let edge2 = graph.create_edge(variable);
        let optional_factor = graph.create_factor(&[edge2], known_value_minimizer(10.0));

        assert_eq!(graph.num_variables(), 1);
        assert_eq!(graph.num_edges(), 2);
        assert_eq!(graph.num_enabled_edges(), 2);
        assert_eq!(graph.num_factors(), 2);
        assert_eq!(graph.num_enabled_factors(), 2);

        graph.set_factor_enabled(optional_factor, false);
        assert!(!graph.is_factor_enabled(optional_factor));
        assert_eq!(graph.num_enabled_factors(), 1);
        assert_eq!(graph.num_enabled_edges(), 1);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 1.0));

        graph.set_factor_enabled(optional_factor, true);
        assert!(graph.is_factor_enabled(optional_factor));
        assert_eq!(graph.num_enabled_factors(), 2);
        assert_eq!(graph.num_enabled_edges(), 2);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 10.0));
    }

    #[test]
    fn iterate_until_satisfied_requires_domain_predicate() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(4.0));

        assert!(!graph.iterate_until_satisfied(10, |graph| graph.value(variable) > 10.0));
        assert!(!graph.converged());
        assert!(nearly_equal(graph.value(variable), 4.0));

        assert!(graph.iterate_until_satisfied(10, |graph| graph.value(variable) == 4.0));
        assert!(graph.converged());
    }

    #[test]
    fn message_difference_remains_available_as_diagnostic() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(7.0));

        assert_eq!(graph.max_message_difference(), None);
        assert!(!graph.iterate());
        assert_eq!(graph.max_message_difference(), None);
        assert!(graph.iterate());
        assert!(
            graph
                .max_message_difference()
                .is_some_and(|diff| diff > 1.0)
        );
        assert!(graph.converged());
    }

    #[test]
    fn graph_callbacks_can_mutate_factor_state() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(1.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        let factor = graph.create_factor(&[edge], known_value_minimizer(10.0));

        graph.add_iteration_graph_callback(Box::new(move |graph| {
            graph.set_factor_enabled(factor, false);
        }));

        assert!(!graph.iterate());
        assert!(!graph.is_factor_enabled(factor));
        assert_eq!(graph.num_enabled_factors(), 0);

        graph.add_reinitialize_graph_callback(Box::new(move |graph| {
            graph.set_factor_enabled(factor, false);
        }));

        graph.reinitialize();
        assert!(!graph.is_factor_enabled(factor));
    }

    #[test]
    fn lone_standard_message_resets_disagreement() {
        let mut graph = FactorGraph::new(0.5, 1e-5, 0);
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let first_edge = graph.create_edge(variable);
        let second_edge = graph.create_edge(variable);

        graph.create_factor(
            &[first_edge],
            Box::new(|exchanges: &mut [_], _: &mut dyn RngCore| {
                exchanges[0].set(WeightedValue::new(10.0, MessageWeight::Standard));
            }),
        );

        use std::cell::Cell;
        use std::rc::Rc;

        let calls = Rc::new(Cell::new(0));
        let calls_clone = Rc::clone(&calls);
        graph.create_factor(
            &[second_edge],
            Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
                if calls_clone.get() == 0 {
                    exchanges[0].set(WeightedValue::new(0.0, MessageWeight::Standard));
                } else {
                    exchanges[0].set(WeightedValue::new(0.0, MessageWeight::Zero));
                }
                calls_clone.set(calls_clone.get() + 1);
            }),
        );

        // After iteration 1: average of 10 and 0 = 5
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 5.0));

        // After iteration 2: second factor emits zero weight → lone standard.
        // Without reset, disagreement would slow convergence.
        // With reset on the lone standard edge, value jumps to 12.5
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 12.5));

        // After iteration 3: converging toward 10.0
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 10.0));
    }

    #[test]
    #[should_panic(expected = "invalid variable")]
    fn invalid_variable_panics() {
        let graph = FactorGraph::default();
        graph.value(VariableNode::new(0));
    }

    #[test]
    #[should_panic(expected = "invalid edge")]
    fn invalid_edge_panics() {
        let mut graph = FactorGraph::default();
        graph.create_variable(0.0, MessageWeight::Standard);
        graph.create_factor(&[GraphEdge::new(3)], Box::new(|_, _| {}));
    }
}
