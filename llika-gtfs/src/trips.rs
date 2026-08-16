//! Which trip represents a route — **OQ-1, and the answer is the modal stop
//! pattern**.
//!
//! A route has many trips: two directions, short-turns, express variants,
//! weekend patterns and one-off specials. Phase 1 shipped the naive answer — the
//! trip with the most stops — deliberately, so this question could be settled
//! against a drawn map, and the map is what says it is wrong: a route's longest
//! trip is often a rare special that runs twice a year and serves a depot, so the
//! line drawn is one no rider has taken.
//!
//! **So a route draws with the pattern the most of its trips run**, which is by
//! construction the line as normally operated. Three things that decides which
//! OQ-1 does not name:
//!
//! **The pattern is the *station* sequence, not the platform sequence.** The
//! claim this makes is that the line *drawn* is the modal one, so two trips that
//! draw the same line have to count as one pattern — and a feed that hands one
//! trip the terminus's bay 1 and the next its bay 2 would otherwise split them.
//! That is exactly the platform noise `stations.rs` collapses to remove, and
//! letting it back in here would dilute the vote it decides.
//!
//! **A tie goes to the pattern whose first trip is the earlier `trips.txt` row**,
//! which is the rule Phase 1 already shipped one level down. Equal-count patterns
//! are the ordinary case rather than the exotic one — two directions of a route
//! usually run the same number of trips — and leaving the choice to enumeration
//! order would make the output file depend on it.
//!
//! **Nothing is excluded from the vote for being too short.** A route whose modal
//! pattern is one station is dropped by `convert.rs` as `LineTooShort` rather
//! than falling back to its longest drawable pattern, because that fallback is
//! the rare-special failure above wearing a different hat: it would crown a
//! pattern one trip in a hundred runs.

use std::collections::HashMap;

use crate::feed::StopTime;

/// The stop ids of a route's representative trip, in `stop_sequence` order.
///
/// `trip_ids` is that route's trips in `trips.txt` row order and
/// `rows_by_trip`'s vectors are in `stop_times.txt` row order — the first is
/// what the tie-break reads, the second what the stable sort below preserves.
/// `station_of` is `stations.rs:collapse_map`, which is what makes two trips
/// through different platforms of one station the same pattern.
///
/// Platform ids come back rather than station ids: `convert.rs` resolves them
/// through `stations.rs:resolve`, which owns the fold and the error for the
/// whole crate.
pub fn representative_stop_ids(
    trip_ids: &[&str],
    rows_by_trip: &HashMap<&str, Vec<&StopTime>>,
    station_of: &HashMap<&str, &str>,
) -> Vec<String> {
    // The distinct patterns in first-appearance order, which is `trips.txt` row
    // order: each is the trip that introduced it and the number of trips that
    // run it. The `HashMap` is lookup only and is never walked to produce
    // output — Rust seeds its default hasher per process, and this is one step
    // upstream of every determinism guarantee `llk-001` proved.
    let mut seen: HashMap<Vec<String>, usize> = HashMap::new();
    let mut patterns: Vec<(&str, usize)> = Vec::new();

    for trip_id in trip_ids {
        let key = pattern(&sorted_rows(trip_id, rows_by_trip), station_of);
        match seen.get(&key) {
            Some(&at) => patterns[at].1 += 1,
            None => {
                seen.insert(key, patterns.len());
                patterns.push((trip_id, 1));
            }
        }
    }

    let mut best: Option<(&str, usize)> = None;
    for &(trip_id, trips) in &patterns {
        // Strictly greater, so a tie leaves the pattern that appeared first —
        // the one whose earliest trip is the earlier `trips.txt` row — standing.
        if best.is_none_or(|(_, incumbent)| trips > incumbent) {
            best = Some((trip_id, trips));
        }
    }

    let Some((trip_id, _)) = best else {
        // A kept route with no trips at all. It becomes a line with no stations,
        // which `convert.rs` drops as `LineTooShort`.
        return Vec::new();
    };

    sorted_rows(trip_id, rows_by_trip)
        .iter()
        .filter_map(|row| row.stop_id.clone())
        .collect()
}

/// One trip's rows in `stop_sequence` order.
///
/// The **value** is the sort key, never the row order and never a difference
/// between values: GTFS requires the sequence to increase along a trip but not
/// to be consecutive, and does not require the file to store the rows in that
/// order. A stable sort, so rows sharing a `stop_sequence` — which GTFS forbids
/// but a feed can still contain — keep their file order rather than an
/// arbitrary one.
fn sorted_rows<'a>(
    trip_id: &str,
    rows_by_trip: &HashMap<&str, Vec<&'a StopTime>>,
) -> Vec<&'a StopTime> {
    let mut rows = rows_by_trip.get(trip_id).cloned().unwrap_or_default();
    rows.sort_by_key(|row| row.stop_sequence);
    rows
}

/// The key two trips are the same pattern by: the stations they serve, in order,
/// with consecutive duplicates folded — the same reading of a trip that
/// `stations.rs:resolve` produces for the winner.
fn pattern(rows: &[&StopTime], station_of: &HashMap<&str, &str>) -> Vec<String> {
    let mut stations: Vec<String> = rows
        .iter()
        .filter_map(|row| row.stop_id.as_deref())
        // Lenient where `resolve` is strict, and deliberately: grouping is a
        // classification, so an id `stops.txt` does not define forms its own
        // bucket rather than failing a route that may not even draw with that
        // trip. If the trip carrying it wins, `resolve` raises `UnknownStop` —
        // the error contract stays in one place.
        .map(|id| (*station_of.get(id).unwrap_or(&id)).to_string())
        .collect();
    stations.dedup();
    stations
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::representative_stop_ids;
    use crate::feed::StopTime;

    /// `(trip_id, [(stop_id, stop_sequence)])` written as a feed would write it,
    /// so a case can put rows out of order the way `stop_times.txt` may.
    fn feed(trips: &[(&str, &[(&str, u32)])]) -> Vec<StopTime> {
        trips
            .iter()
            .flat_map(|(trip_id, rows)| {
                rows.iter().map(move |(stop_id, stop_sequence)| StopTime {
                    trip_id: (*trip_id).to_string(),
                    stop_id: Some((*stop_id).to_string()),
                    stop_sequence: *stop_sequence,
                })
            })
            .collect()
    }

    /// Everything `representative_stop_ids` reads, assembled the way
    /// `convert.rs` assembles it.
    fn choose<'a>(
        trip_ids: &[&str],
        rows: &'a [StopTime],
        parents: &[(&'a str, &'a str)],
    ) -> Vec<String> {
        let mut rows_by_trip: HashMap<&str, Vec<&StopTime>> = HashMap::new();
        for row in rows {
            rows_by_trip
                .entry(row.trip_id.as_str())
                .or_default()
                .push(row);
        }
        let station_of: HashMap<&str, &str> = parents.iter().copied().collect();
        representative_stop_ids(trip_ids, &rows_by_trip, &station_of)
    }

    /// The whole point of the phase, at the smallest size that shows it: three
    /// trips run the short pattern and one runs a longer special, and the
    /// special is not the line.
    #[test]
    fn the_modal_pattern_beats_the_longest_trip() {
        let rows = feed(&[
            ("t1", &[("A", 1), ("B", 2), ("C", 3)]),
            ("t2", &[("A", 1), ("B", 2), ("C", 3)]),
            ("special", &[("A", 1), ("B", 2), ("C", 3), ("D", 4), ("E", 5)]),
            ("t3", &[("A", 1), ("B", 2), ("C", 3)]),
        ]);

        assert_eq!(
            choose(&["t1", "t2", "special", "t3"], &rows, &[]),
            ["A", "B", "C"],
        );
    }

    /// The tie-break, and it decides a case that is ordinary rather than exotic:
    /// two directions of a route usually run the same number of trips.
    #[test]
    fn a_tie_goes_to_the_earlier_trips_txt_row() {
        let rows = feed(&[
            ("out1", &[("A", 1), ("B", 2)]),
            ("back1", &[("B", 1), ("A", 2)]),
            ("out2", &[("A", 1), ("B", 2)]),
            ("back2", &[("B", 1), ("A", 2)]),
        ]);

        assert_eq!(
            choose(&["out1", "back1", "out2", "back2"], &rows, &[]),
            ["A", "B"],
        );
        // The order the ids are handed in is the order that decides it, not the
        // order they happen to hash in.
        assert_eq!(
            choose(&["back1", "out1", "out2", "back2"], &rows, &[]),
            ["B", "A"],
        );
    }

    /// **The rule the fixture cannot witness**, and the reason the collapse map
    /// is a parameter at all: two trips down the same line that a feed gave
    /// different platforms of one station are *one* pattern, so their votes
    /// combine and outweigh a longer special.
    ///
    /// Keyed on the raw platform sequence they would be two patterns of one trip
    /// each, tied with the special, and the tie-break would draw whichever came
    /// first — the line as normally operated losing to an accident of bay
    /// assignment.
    #[test]
    fn two_trips_differing_only_in_platform_are_one_pattern() {
        let rows = feed(&[
            ("bay1", &[("A", 1), ("X_1", 2), ("B", 3)]),
            ("special", &[("A", 1), ("X_1", 2), ("B", 3), ("C", 4)]),
            ("bay2", &[("A", 1), ("X_2", 2), ("B", 3)]),
        ]);
        let parents = [("X_1", "X"), ("X_2", "X"), ("A", "A"), ("B", "B"), ("C", "C")];

        // The winner is a trip, so its own platform ids come back; `resolve`
        // turns them into stations. What this asserts is which trip won.
        assert_eq!(
            choose(&["bay1", "special", "bay2"], &rows, &parents),
            ["A", "X_1", "B"],
        );
    }

    /// Two trips of one pattern, one of them written out of `stop_sequence`
    /// order. The rows are sorted **before** the key is taken, so the file's row
    /// order cannot split a pattern in two.
    #[test]
    fn rows_are_sorted_before_two_trips_are_compared() {
        let rows = feed(&[
            ("tidy", &[("A", 10), ("B", 20), ("C", 30)]),
            ("special", &[("A", 1), ("B", 2), ("C", 3), ("D", 4)]),
            ("jumbled", &[("C", 30), ("A", 10), ("B", 20)]),
        ]);

        assert_eq!(
            choose(&["tidy", "special", "jumbled"], &rows, &[]),
            ["A", "B", "C"],
        );
    }

    /// The degenerate cases every selection rule has to survive: one trip, and
    /// none at all. A route with no trips gets no stations and `convert.rs`
    /// drops it.
    #[test]
    fn one_trip_is_its_own_pattern_and_no_trips_is_no_line() {
        let rows = feed(&[("only", &[("A", 1), ("B", 2)])]);

        assert_eq!(choose(&["only"], &rows, &[]), ["A", "B"]);
        assert!(choose(&[], &rows, &[]).is_empty());
    }
}
