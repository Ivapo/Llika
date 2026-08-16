//! Which trip represents a route.
//!
//! **This phase ships a deliberately naive answer: the trip with the most
//! stops.** The spec's OQ-1 says in terms that it is wrong — a route's longest
//! trip is often a rare special that runs twice a year and serves a depot, so
//! the line drawn is one no rider has taken — and Phase 3 replaces it. It ships
//! anyway so that question is decided against a drawn map rather than in the
//! abstract.
//!
//! The tie-break is not naive, and is the one Phase 3 will need anyway: two
//! equal-length direction variants are the ordinary case rather than the exotic
//! one, and leaving the choice to enumeration order would make the output file
//! depend on it.

use std::collections::HashMap;

use crate::feed::StopTime;

/// The stop ids of a route's representative trip, in `stop_sequence` order.
///
/// `trip_ids` is that route's trips in `trips.txt` row order, and
/// `rows_by_trip`'s vectors are in `stop_times.txt` row order — both matter, the
/// first for the tie-break and the second for the stable sort below.
pub fn representative_stop_ids(
    trip_ids: &[&str],
    rows_by_trip: &HashMap<&str, Vec<&StopTime>>,
) -> Vec<String> {
    let mut best: Option<(&str, usize)> = None;
    for trip_id in trip_ids {
        let count = rows_by_trip.get(trip_id).map_or(0, Vec::len);
        // Strictly greater, so a tie leaves the earlier `trips.txt` row standing.
        if best.is_none_or(|(_, incumbent)| count > incumbent) {
            best = Some((trip_id, count));
        }
    }

    let Some((trip_id, _)) = best else {
        // A kept route with no trips at all. It becomes a line with no stations,
        // which `llika-core/src/io.rs:Network::from_input` rejects as
        // `LineTooShort` — the condition OQ-3 owns and Phase 2 answers.
        return Vec::new();
    };

    let mut rows = rows_by_trip.get(trip_id).cloned().unwrap_or_default();
    // A stable sort, so rows sharing a `stop_sequence` — which GTFS forbids but
    // a feed can still contain — keep their file order rather than an arbitrary
    // one.
    rows.sort_by_key(|row| row.stop_sequence);
    rows.iter().filter_map(|row| row.stop_id.clone()).collect()
}
