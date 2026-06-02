# Circle Packing

Circle packing uses the factor graph to move circle centers until every circle
fits inside a rectangular coordinate range and no two circles overlap.

The implementation lives in
[src/problems/circle_packing.rs.md](../src/problems/circle_packing.rs.md).
That source note covers circle generation, the direct builder, the fast
builder, boundary constraints, intersection constraints, kiss constraints,
state extraction, overlap measurement, and dynamic factor management.

## Variables

Each circle contributes two variables:

```text
x_i = horizontal center coordinate
y_i = vertical center coordinate
```

The radius is not a variable. It is fixed input data used by the factors that
constrain the center coordinates.

The returned `CirclePackingVariables` object stores the `x` and `y` variable
handles plus the fixed radii. See
[src/problems/circle_packing.rs.md](../src/problems/circle_packing.rs.md).

## Boundary Factors

For a circle of radius `r`, the horizontal center must stay in:

```text
[horizontal.lower + r, horizontal.upper - r]
```

The vertical center follows the same rule. The builder uses the existing
[`src/minimizers/in_range.rs.md`](../src/minimizers/in_range.rs.md) factor for
these constraints. A center inside the range receives a zero-weight message; a
center outside the range receives a standard-weight clamp to the nearest valid
coordinate.

## Intersection Factors

For two circles with centers `(x1, y1)` and `(x2, y2)`, the required center
distance is:

```text
r1 + r2
```

The intersection factor has four edges: `x1`, `y1`, `x2`, and `y2`. When the
centers are already at least `r1 + r2` apart, the factor emits zero-weight
messages and leaves the incoming coordinates alone. When the circles overlap,
it moves the proposed centers apart symmetrically along the line between the
centers.

If the centers exactly coincide, the factor chooses the positive x direction as
the deterministic separating direction.

## Kiss Factors

A kiss factor constrains a circle center to lie at an exact distance from a
fixed point. This is useful when circles should be tangent to a fixed circle:
the exact center distance is the fixed circle radius plus the moving circle
radius.

As with coincident intersection centers, a circle initially at the fixed point
uses the positive x direction as the deterministic direction.

## Fast Intersection Management

The direct builder creates and enables every pairwise intersection factor. That
is simple and robust, but the number of pair factors grows quadratically.

The fast builder still allocates every pair factor so the graph shape remains
stable, but it disables factors for circle pairs that are clearly far apart. A
small spatial grid groups circles by current center position. After each
iteration, graph-aware callbacks update which pair factors are enabled.

The grid check is conservative: nearby pairs can be enabled even before they
overlap. This keeps the solver from missing collisions while avoiding most
distant pair work.

The dynamic enable/disable mechanism uses graph-aware callbacks on
`FactorGraph`; those callbacks are explained in
[src/factor_graph.rs.md](../src/factor_graph.rs.md). The circle-packing
manager that decides which pairs are active is explained in
[src/problems/circle_packing.rs.md](../src/problems/circle_packing.rs.md).

## Measuring Progress

[`src/problems/circle_packing.rs.md`](../src/problems/circle_packing.rs.md)
provides `max_overlap`, which reports the largest current violation among:

- left, right, bottom, and top boundary constraints;
- all pairwise circle overlaps.

`0.0` means the current graph values describe a valid packing.

## Relationship To TWA

Circle packing is the clearest example in this crate of zero-weight messages
as "no active opinion." A satisfied boundary or intersection factor emits zero
weight, so it does not pull the variable consensus. An active violation emits
standard weight, so the variable pass averages it with other active geometric
corrections.

The optional kiss constraint is different: it asserts an exact tangent
relationship and therefore keeps sending standard-weight messages.

For the weight rules, read [weights.md](weights.md). For the iteration loop,
read [message_passing.md](message_passing.md).

## Further Reading

- [src/problems/circle_packing.rs.md](../src/problems/circle_packing.rs.md)
  explains the full implementation.
- [src/minimizers/in_range.rs.md](../src/minimizers/in_range.rs.md) explains
  the scalar boundary minimizer used for circle coordinates.
- [src/factor_graph.rs.md](../src/factor_graph.rs.md) explains the dynamic
  factor enablement API used by the fast builder.
- [src/bin/gui.rs.md](../src/bin/gui.rs.md) shows how the GUI steps the graph
  and visualizes overlap, active pairs, and convergence.
