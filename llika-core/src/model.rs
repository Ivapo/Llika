//! The data model: stations, lines, and the graph that joins them.
//!
//! Two invariants live here and everything downstream leans on them:
//!
//! 1. **Two lines over the same consecutive pair share one edge.** The edge is
//!    the corridor, so a shared trunk is a topological fact rather than a
//!    geometric guess.
//! 2. **Input order is the iteration order everywhere it is observable.**
//!    `stations` and `lines` are vectors in input-file order and node indices
//!    coincide with `stations` indices; the `HashMap` below exists for lookup
//!    and is never iterated. A hash map walked to produce output would make the
//!    SVG vary between processes, since Rust seeds its default hasher per
//!    process.

use std::collections::{BTreeSet, HashMap};

use petgraph::graph::{NodeIndex, UnGraph};
use serde::{Deserialize, Serialize};

/// One station, with its real-world position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

/// One line, as an **ordered** station list. The graph's edges are the
/// consecutive pairs of that list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: String,
    pub name: String,
    pub color: String,
    pub stations: Vec<String>,
}

/// The lines crossing one edge, as indices into [`Network::lines`].
///
/// A `BTreeSet` and not a `HashSet`: line indices are input order, and this set
/// is iterated to produce output.
pub type LineSet = BTreeSet<usize>;

/// The station graph. Nodes are stations, edges are corridors.
#[derive(Debug, Clone)]
pub struct Network {
    pub(crate) stations: Vec<Station>,
    pub(crate) lines: Vec<Line>,
    pub(crate) graph: UnGraph<usize, LineSet>,
    /// Lookup only — never iterated. See the module invariant.
    pub(crate) index_by_id: HashMap<String, usize>,
}

impl Network {
    /// Stations in input-file order.
    pub fn stations(&self) -> &[Station] {
        &self.stations
    }

    /// Lines in input-file order.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn graph(&self) -> &UnGraph<usize, LineSet> {
        &self.graph
    }

    /// The deduped edge count: corridors, not line-segments.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// The index of a station id, or `None`. Lookup, not iteration.
    pub fn station_index(&self, id: &str) -> Option<usize> {
        self.index_by_id.get(id).copied()
    }

    /// The graph node for a station index. Node indices coincide with station
    /// indices by construction — nodes are added in input order.
    pub fn node(&self, station: usize) -> NodeIndex {
        NodeIndex::new(station)
    }

    /// The number of corridors meeting at a station. Two lines sharing a
    /// corridor count once, which is what makes degree 2 mean "passes through".
    pub fn degree(&self, station: usize) -> usize {
        self.graph.edges(self.node(station)).count()
    }

    /// The union of the line sets on a station's incident edges.
    pub fn lines_at_station(&self, station: usize) -> LineSet {
        self.graph
            .edges(self.node(station))
            .flat_map(|e| e.weight().iter().copied())
            .collect()
    }

    /// The lines crossing the corridor between two stations, or `None` when
    /// they are not adjacent.
    pub fn lines_between(&self, a: usize, b: usize) -> Option<&LineSet> {
        let edge = self.graph.find_edge(self.node(a), self.node(b))?;
        self.graph.edge_weight(edge)
    }
}
