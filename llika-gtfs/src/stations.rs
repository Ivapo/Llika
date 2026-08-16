//! Platforms collapse into stations — §2.2, the load-bearing problem.
//!
//! A GTFS `stop_id` is usually a **platform**, not a station. A metro stop with
//! two directions is two `stop_id`s; an interchange served by three lines can be
//! six. Imported naively that destroys the one invariant `llk-001` is built on:
//! `llika-core/src/io.rs:Network::from_input` makes an edge *the corridor*, and
//! two lines through one station on two different platform ids never produce the
//! same consecutive pair. So `llika-core/src/model.rs:LineSet` stays a singleton
//! everywhere and the line-bundling renderer has nothing to bundle,
//! `llika-core/src/model.rs:degree` counts platforms so a four-line interchange
//! reads as four degree-2 stations, and the map shows every station twice.
//!
//! **A station is the thing a rider changes at, and that is the
//! `location_type = 1` row.** The collapse is not an option and not a flag.
//!
//! It creates one problem of its own, and that is what [`resolve`]'s fold is
//! for: a trip serving two platforms of one station in sequence collapses to the
//! same id twice consecutively, which
//! `llika-core/src/io.rs:InputError::RepeatedStation` rejects. Folding is not
//! data loss — the rider is at one station — and it is the only one of
//! `llk-001`'s five conditions that is an artefact of this step rather than a
//! defect in the feed.

use std::collections::HashMap;

use crate::ImportError;
use crate::feed::Stop;

/// The `stop_id` of the station each read stop collapses into: its
/// `location_type = 1` parent, or itself where it has none.
///
/// **One hop, never a chain.** GTFS forbids a `location_type = 1` row from
/// carrying a `parent_station`, so a station row always maps to itself and a
/// platform's parent is final. Rows §2.1 does not read — entrances, generic
/// nodes, boarding areas — are absent from the map entirely, which is what makes
/// "not in the map" the single check [`resolve`] needs.
///
/// A platform with no `parent_station` is its own station, which is the correct
/// reading of a feed that does not model parents at all.
pub fn collapse_map(stops: &[Stop]) -> Result<HashMap<&str, &str>, ImportError> {
    let by_id: HashMap<&str, &Stop> = stops
        .iter()
        .filter(|stop| stop.is_stop_or_station())
        .map(|stop| (stop.stop_id.as_str(), stop))
        .collect();

    let mut station_of: HashMap<&str, &str> = HashMap::with_capacity(by_id.len());
    for stop in stops.iter().filter(|stop| stop.is_stop_or_station()) {
        let id = stop.stop_id.as_str();

        // A station row is its own station. Applying the parent rule to it would
        // build the chain GTFS says cannot exist.
        if stop.location_type == Some(1) {
            station_of.insert(id, id);
            continue;
        }

        let parent = match stop.parent_station.as_deref() {
            Some(parent) if !parent.is_empty() => parent,
            _ => {
                station_of.insert(id, id);
                continue;
            }
        };

        // A `parent_station` naming something that is not a station is a
        // malformed feed, and it is a hard error for §2.4's reason: the
        // alternative is falling back to the platform's own id, which silently
        // reproduces the duplicated interchange this whole module exists to
        // prevent, and reproduces it invisibly.
        match by_id.get(parent) {
            Some(row) if row.location_type == Some(1) => {
                station_of.insert(id, row.stop_id.as_str());
            }
            _ => {
                return Err(ImportError::UnknownParentStation {
                    stop_id: stop.stop_id.clone(),
                    parent: parent.to_string(),
                });
            }
        }
    }

    Ok(station_of)
}

/// One line's station list: every platform id resolved to its station, then
/// consecutive duplicates folded to one.
///
/// The fold is what keeps the collapse from producing a self-loop. `dedup`
/// removes only *consecutive* duplicates, which is exactly §2.2's rule — a line
/// that legitimately returns to a station later in its run keeps both visits.
pub fn resolve(
    platform_ids: &[String],
    station_of: &HashMap<&str, &str>,
    route: &str,
) -> Result<Vec<String>, ImportError> {
    let mut stations = Vec::with_capacity(platform_ids.len());

    for id in platform_ids {
        // The map holds every row §2.1 reads, so a miss is exactly "`stops.txt`
        // omits this id, or it resolves to a row this importer does not read".
        // Both mean a malformed feed, and both are errors rather than skips: a
        // skip cascades into `InputError::UnknownStation` one step later, where
        // the message names the wrong problem.
        let Some(station) = station_of.get(id.as_str()) else {
            return Err(ImportError::UnknownStop {
                route: route.to_string(),
                stop_id: id.clone(),
            });
        };
        stations.push((*station).to_string());
    }

    stations.dedup();
    Ok(stations)
}

#[cfg(test)]
mod tests {
    use super::{collapse_map, resolve};
    use crate::ImportError;
    use crate::feed::Stop;

    fn stop(stop_id: &str, location_type: Option<u8>, parent_station: Option<&str>) -> Stop {
        Stop {
            stop_id: stop_id.to_string(),
            stop_name: None,
            stop_lat: Some(0.0),
            stop_lon: Some(0.0),
            location_type,
            parent_station: parent_station.map(str::to_string),
        }
    }

    /// The three readings of a `stops.txt` row, and the row that is not read at
    /// all. The gate asserts this against the fixture; this is the cheaper
    /// statement of the same rule, and it is what makes a failure there readable.
    fn fixture() -> Vec<Stop> {
        vec![
            stop("CEN", Some(1), None),
            stop("CEN_1", Some(0), Some("CEN")),
            stop("CEN_E1", Some(2), Some("CEN")),
            stop("WST", Some(0), None),
            stop("EAST", None, Some("")),
        ]
    }

    #[test]
    fn a_platform_collapses_to_its_parent_and_everything_else_is_its_own_station() {
        let stops = fixture();
        let station_of = collapse_map(&stops).expect("a conforming feed");

        assert_eq!(station_of.get("CEN_1"), Some(&"CEN"));
        // A station row is its own station, never a chain.
        assert_eq!(station_of.get("CEN"), Some(&"CEN"));
        // No `parent_station` at all, and an empty cell, are the same reading.
        assert_eq!(station_of.get("WST"), Some(&"WST"));
        assert_eq!(station_of.get("EAST"), Some(&"EAST"));
        // §2.1 does not read an entrance, so it is not a collapse target either.
        assert_eq!(station_of.get("CEN_E1"), None);
    }

    #[test]
    fn a_parent_station_that_names_no_station_is_an_error() {
        for dangling in [
            // Nothing defines it.
            stop("X_1", Some(0), Some("NOWHERE")),
            // It is defined, but as a platform rather than a station.
            stop("X_2", Some(0), Some("WST")),
            // It is defined, but as a row §2.1 does not read.
            stop("X_3", Some(0), Some("CEN_E1")),
        ] {
            let id = dangling.stop_id.clone();
            let mut stops = fixture();
            stops.push(dangling);

            assert!(
                matches!(
                    collapse_map(&stops),
                    Err(ImportError::UnknownParentStation { ref stop_id, .. }) if *stop_id == id
                ),
                "`{id}` should not have resolved",
            );
        }
    }

    #[test]
    fn consecutive_platforms_of_one_station_fold_and_a_later_return_does_not() {
        let stops = fixture();
        let station_of = collapse_map(&stops).expect("a conforming feed");
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        // Two platforms of one station in sequence are one visit.
        assert_eq!(
            resolve(&ids(&["WST", "CEN_1", "CEN", "EAST"]), &station_of, "R").unwrap(),
            ["WST", "CEN", "EAST"],
        );
        // A line that comes back to a station later keeps both visits: `dedup`
        // folds only what is adjacent.
        assert_eq!(
            resolve(&ids(&["CEN_1", "WST", "CEN"]), &station_of, "R").unwrap(),
            ["CEN", "WST", "CEN"],
        );
    }

    #[test]
    fn a_stop_the_feed_does_not_define_is_an_error() {
        let stops = fixture();
        let station_of = collapse_map(&stops).expect("a conforming feed");

        for unreadable in ["GHOST", "CEN_E1"] {
            let list = vec!["WST".to_string(), unreadable.to_string()];
            assert!(
                matches!(
                    resolve(&list, &station_of, "R"),
                    Err(ImportError::UnknownStop { ref stop_id, .. }) if stop_id == unreadable
                ),
                "`{unreadable}` should not have resolved",
            );
        }
    }
}
