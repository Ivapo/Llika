//! The input file and its validation.
//!
//! Five conditions are hard errors. None is a warning and none is silently
//! repaired, because every one of them makes a downstream invariant
//! unenforceable — see the spec's §2.1. A station defined but referenced by no
//! line is **legal**: an isolated node is a degenerate but drawable map.

use std::collections::{HashMap, HashSet};

use petgraph::graph::{NodeIndex, UnGraph};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{Line, LineSet, Network, Station};

/// The whole input file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputSchema {
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
}

/// The five conditions [`Network::from_input`] rejects.
#[derive(Debug, Error, PartialEq)]
pub enum InputError {
    /// There would be no node to attach the edge to.
    #[error("line `{line}` references station `{station}`, which is not defined")]
    UnknownStation { line: String, station: String },

    /// Node identity would stop being a key.
    #[error("duplicate station id `{id}`")]
    DuplicateStation { id: String },

    /// Line identity is the render-offset sort key.
    #[error("duplicate line id `{id}`")]
    DuplicateLine { id: String },

    /// Contributes no edge, so it would silently vanish from the map.
    #[error("line `{id}` lists {count} station(s); a line needs at least 2")]
    LineTooShort { id: String, count: usize },

    /// A self-loop, which has no angle and breaks `c3`/`c5`.
    #[error("line `{line}` repeats station `{station}` consecutively")]
    RepeatedStation { line: String, station: String },
}

/// Parse an input file. Syntax errors are `serde_json`'s; the five semantic
/// conditions above belong to [`Network::from_input`], which is why they are
/// separate types.
pub fn parse_input(json: &str) -> Result<InputSchema, serde_json::Error> {
    serde_json::from_str(json)
}

impl Network {
    /// Validate an input file and build the graph.
    ///
    /// Checks run in a fixed order — duplicate station ids, duplicate line ids,
    /// then per line in input order: too short, consecutive repeat, unknown
    /// reference — so an input violating two conditions always reports the same
    /// one.
    pub fn from_input(input: &InputSchema) -> Result<Self, InputError> {
        let mut index_by_id: HashMap<String, usize> = HashMap::new();
        for (i, station) in input.stations.iter().enumerate() {
            if index_by_id.insert(station.id.clone(), i).is_some() {
                return Err(InputError::DuplicateStation {
                    id: station.id.clone(),
                });
            }
        }

        let mut line_ids: HashSet<&str> = HashSet::new();
        for line in &input.lines {
            if !line_ids.insert(&line.id) {
                return Err(InputError::DuplicateLine {
                    id: line.id.clone(),
                });
            }
        }

        let mut graph: UnGraph<usize, LineSet> = UnGraph::default();
        for i in 0..input.stations.len() {
            graph.add_node(i);
        }

        for (line_index, line) in input.lines.iter().enumerate() {
            if line.stations.len() < 2 {
                return Err(InputError::LineTooShort {
                    id: line.id.clone(),
                    count: line.stations.len(),
                });
            }
            for pair in line.stations.windows(2) {
                let (from, to) = (&pair[0], &pair[1]);
                if from == to {
                    return Err(InputError::RepeatedStation {
                        line: line.id.clone(),
                        station: from.clone(),
                    });
                }
                let a =
                    *index_by_id
                        .get(from.as_str())
                        .ok_or_else(|| InputError::UnknownStation {
                            line: line.id.clone(),
                            station: from.clone(),
                        })?;
                let b =
                    *index_by_id
                        .get(to.as_str())
                        .ok_or_else(|| InputError::UnknownStation {
                            line: line.id.clone(),
                            station: to.clone(),
                        })?;

                // The edge is the corridor: a second line over the same pair
                // joins the existing edge rather than adding one.
                let (na, nb) = (NodeIndex::new(a), NodeIndex::new(b));
                match graph.find_edge(na, nb) {
                    Some(edge) => {
                        graph[edge].insert(line_index);
                    }
                    None => {
                        graph.add_edge(na, nb, LineSet::from([line_index]));
                    }
                }
            }
        }

        Ok(Network {
            stations: input.stations.clone(),
            lines: input.lines.clone(),
            graph,
            index_by_id,
        })
    }
}
