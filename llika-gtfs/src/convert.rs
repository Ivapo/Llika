//! `Feed` → `llika-core/src/io.rs:InputSchema`.
//!
//! Two rules govern this file and both are load-bearing.
//!
//! **A station is emitted iff some kept line's station list references it.** So
//! a `location_type = 1` parent row is read — Phase 2 collapses platforms into
//! it — but emits nothing on its own account, and a stop serving only a
//! filtered-out route emits nothing either. Without the rule those rows draw as
//! stray isolated markers, which `Network::from_input` accepts as legal and
//! which nobody wants on a poster.
//!
//! **Input order is the iteration order.** Stations come out in `stops.txt` row
//! order and lines in `routes.txt` row order. The `HashMap`s below are built for
//! lookup and are never walked to produce output: Rust seeds its default hasher
//! per process, so a map iterated into the `stations` array would give a
//! different file every run and evaporate every downstream guarantee `llk-001`
//! proved.

use std::collections::{HashMap, HashSet};

use llika_core::{InputSchema, Line, Station};

use crate::feed::{Feed, Stop, StopTime};
use crate::trips::representative_stop_ids;
use crate::{ImportError, ImportParams, ImportReport};

/// The fallback colours, in order, for a route whose `route_color` is absent or
/// malformed. The first three are `sample_network.json`'s, so an imported map
/// reads in the same register as the hand-authored one.
pub const FALLBACK_PALETTE: [&str; 8] = [
    "#E4002B", "#00843D", "#0057B8", "#FF8200", "#753BBD", "#00A3E0", "#FFB81C", "#7C878E",
];

pub fn to_schema(
    feed: &Feed,
    params: &ImportParams,
) -> Result<(InputSchema, ImportReport), ImportError> {
    let stops_by_id: HashMap<&str, &Stop> = feed
        .stops
        .iter()
        .map(|stop| (stop.stop_id.as_str(), stop))
        .collect();

    let mut rows_by_trip: HashMap<&str, Vec<&StopTime>> = HashMap::new();
    for row in &feed.stop_times {
        rows_by_trip
            .entry(row.trip_id.as_str())
            .or_default()
            .push(row);
    }

    let mut trips_by_route: HashMap<&str, Vec<&str>> = HashMap::new();
    for trip in &feed.trips {
        trips_by_route
            .entry(trip.route_id.as_str())
            .or_default()
            .push(trip.trip_id.as_str());
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut referenced: HashSet<&str> = HashSet::new();
    // Counted over the kept routes that need a fallback, so every colour in the
    // palette is used before any repeats.
    let mut fallbacks_used = 0usize;

    for route in feed
        .routes
        .iter()
        .filter(|route| params.route_types.contains(&route.route_type))
    {
        let trip_ids = trips_by_route
            .get(route.route_id.as_str())
            .cloned()
            .unwrap_or_default();
        let stations = representative_stop_ids(&trip_ids, &rows_by_trip);

        for id in &stations {
            match stops_by_id.get(id.as_str()) {
                Some(stop) if stop.is_stop_or_station() => {
                    referenced.insert(stop.stop_id.as_str());
                }
                // A `stop_times` row naming an id `stops.txt` omits, or one that
                // resolves to a row §2.1 does not read. Both mean a malformed
                // feed, and both are errors here rather than skips: a skip
                // cascades into `InputError::UnknownStation` one step later,
                // where the message names the wrong problem.
                _ => {
                    return Err(ImportError::UnknownStop {
                        route: route.route_id.clone(),
                        stop_id: id.clone(),
                    });
                }
            }
        }

        let color = match stated_color(route.route_color.as_deref()) {
            Some(color) => color,
            None => {
                let color = FALLBACK_PALETTE[fallbacks_used % FALLBACK_PALETTE.len()].to_string();
                fallbacks_used += 1;
                color
            }
        };

        lines.push(Line {
            id: route.route_id.clone(),
            name: route.name().to_string(),
            color,
            stations,
        });
    }

    let mut stations: Vec<Station> = Vec::new();
    for stop in &feed.stops {
        if !referenced.contains(stop.stop_id.as_str()) {
            continue;
        }
        // GTFS makes the coordinates Conditionally Required and the condition is
        // satisfied for exactly the rows kept here, so an empty cell is a
        // malformed feed. A hard error naming the stop, rather than a skip that
        // becomes `InputError::UnknownStation`, or a silent `(0, 0)` that puts a
        // station in the Gulf of Guinea and drags the whole centroid with it.
        let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
            return Err(ImportError::MissingCoordinates {
                stop_id: stop.stop_id.clone(),
            });
        };
        stations.push(Station {
            id: stop.stop_id.clone(),
            name: stop.name().to_string(),
            lat,
            lon,
        });
    }

    let report = ImportReport {
        routes_seen: feed.routes.len(),
        routes_kept: lines.len(),
        stops_seen: feed.stops.len(),
        stations_emitted: stations.len(),
    };

    Ok((InputSchema { stations, lines }, report))
}

/// The colour the feed actually stated, or `None` when the fallback should fire.
///
/// The predicate is exactly §2.4's: the cell is missing or empty, or it is not
/// six hexadecimal digits. **An explicit `FFFFFF` is kept as white.** GTFS makes
/// an omitted `route_color` default to `FFFFFF` by the standard, so "missing"
/// and "white" are one value downstream and two different cells in the file —
/// and they are distinguished at the cell. §1.1 assigns the editorial judgement
/// about a white line to the person editing the intermediate file, not to this
/// function.
///
/// The `#` is prefixed because `llika-core/src/model.rs:Line`'s `color` goes
/// straight into an SVG `stroke` attribute, and GTFS writes the six digits bare.
fn stated_color(cell: Option<&str>) -> Option<String> {
    let digits = cell?;
    (digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("#{digits}"))
}

#[cfg(test)]
mod tests {
    use super::stated_color;

    /// The one conversion whose two branches look alike and are not. A gate
    /// assertion covers both ends of it on the fixture; this is the cheaper
    /// statement of the same rule, and it is what makes a failure there readable.
    #[test]
    fn a_stated_colour_survives_and_only_a_malformed_one_falls_back() {
        assert_eq!(stated_color(Some("FFFFFF")).as_deref(), Some("#FFFFFF"));
        // Case is the feed's to choose; nothing here normalises it.
        assert_eq!(stated_color(Some("0057b8")).as_deref(), Some("#0057b8"));

        for absent_or_malformed in [None, Some(""), Some("#0057B8"), Some("57B8"), Some("GGGGGG")] {
            assert_eq!(
                stated_color(absent_or_malformed),
                None,
                "{absent_or_malformed:?} must fall back"
            );
        }
    }
}
