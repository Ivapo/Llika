//! `Feed` → `llika-core/src/io.rs:InputSchema`.
//!
//! Two rules govern this file and both are load-bearing.
//!
//! **A station is emitted iff some kept line's station list references it**, and
//! the reference is whatever identity is current — since `stations.rs` collapses
//! platforms, that is the parent's id. So the platform rows are read and emit
//! nothing on their own account, and a stop serving only a filtered-out or
//! dropped route emits nothing either. Without the rule those rows draw as
//! stray isolated markers, which `Network::from_input` accepts as legal and
//! which nobody wants on a poster.
//!
//! The rule is also what puts a collapsed station at its **parent's** row rather
//! than its first platform's: the emission loop walks `stops.txt` in row order
//! and the parent row is the one whose id is now referenced. That is a picture
//! decision and not a tidiness one — `llk-001` §2.2 resolves two stations
//! rounding to one grid cell by `stations` array order, first claim wins, so the
//! two readings hand the layout the same network in different orders and get
//! different maps.
//!
//! **Input order is the iteration order.** Stations come out in `stops.txt` row
//! order and lines in `routes.txt` row order. The `HashMap`s below are built for
//! lookup and are never walked to produce output: Rust seeds its default hasher
//! per process, so a map iterated into the `stations` array would give a
//! different file every run and evaporate every downstream guarantee `llk-001`
//! proved.

use std::collections::{HashMap, HashSet};

use llika_core::{InputSchema, Line, Station};

use crate::feed::{Feed, StopTime};
use crate::stations::{collapse_map, resolve};
use crate::trips::representative_stop_ids;
use crate::{DropReason, DroppedRoute, ImportError, ImportParams, ImportReport, MergedRoute};

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
    // Every read stop, mapped onto the station a rider changes at. Built once
    // for the whole feed: it is a property of `stops.txt`, not of any route.
    let station_of = collapse_map(&feed.stops)?;

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
    let mut referenced: HashSet<String> = HashSet::new();
    let mut routes_kept = 0usize;
    let mut routes_dropped: Vec<DroppedRoute> = Vec::new();
    let mut routes_merged: Vec<MergedRoute> = Vec::new();
    // Counted over the kept routes that need a fallback, so every colour in the
    // palette is used before any repeats.
    let mut fallbacks_used = 0usize;

    for route in feed
        .routes
        .iter()
        .filter(|route| params.route_types.contains(&route.route_type))
    {
        routes_kept += 1;

        let trip_ids = trips_by_route
            .get(route.route_id.as_str())
            .cloned()
            .unwrap_or_default();
        let platform_ids = representative_stop_ids(&trip_ids, &rows_by_trip, &station_of);
        let stations = resolve(&platform_ids, &station_of, &route.route_id)?;

        // OQ-3. The drop comes before anything else this iteration touches, and
        // both halves of that matter: §2.1 says a dropped route is not a kept
        // line, so it references nothing and contributes no stations — the
        // station it uniquely served would otherwise emit as exactly the stray
        // isolated marker the emit rule exists to prevent — and a route that
        // never becomes a line does not consume a palette index either.
        if stations.len() < 2 {
            routes_dropped.push(DroppedRoute {
                route_id: route.route_id.clone(),
                reason: DropReason::LineTooShort {
                    stations: stations.len(),
                },
            });
            continue;
        }

        // A route that draws a line an earlier route already drew is absorbed
        // into it. This is checked in the same place and for the same reasons as
        // the drop above: an absorbed route references no station the survivor
        // has not already referenced, and it takes no palette index, so the
        // fallback colours still run in order across the routes that become
        // lines.
        if let Some(into) = lines
            .iter()
            .find(|line| draws_the_same_line(&line.stations, &stations))
        {
            routes_merged.push(MergedRoute {
                route_id: route.route_id.clone(),
                into: into.id.clone(),
            });
            continue;
        }

        referenced.extend(stations.iter().cloned());

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
        // A collapsed station takes its parent row's name and coordinates too,
        // which is what makes it "Central" rather than "Central Platform 1".
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
        routes_kept,
        routes_dropped,
        routes_merged,
        stops_seen: feed.stops.len(),
        stations_emitted: stations.len(),
    };

    Ok((InputSchema { stations, lines }, report))
}

/// Whether a candidate route would draw a line already drawn.
///
/// **Equal or exactly reversed**, and the predicate is structural on purpose: it
/// reads no name, no colour and no `direction_id`, so §2.1's column table is
/// untouched by it. A feed may publish each direction of one line as its own
/// route — BART's `Yellow-N` and `Yellow-S` — and the second contributes no
/// consecutive pair the first did not, since
/// `llika-core/src/model.rs:LineSet` keys a corridor on an *unordered* pair.
///
/// **It does not follow that the map is unchanged, and measuring that was worth
/// the trouble.** The graph is identical, so `c1`, `c2`, `c3` and `c5` are; but
/// `llika-core/src/layout/cost.rs:c4_straightness` sums over **lines**, so a
/// route published twice pays its bends twice and the search weights its
/// straightness by how many times its operator wrote it down. On BART that is
/// not a rounding difference — 50 stations converge to a different arrangement,
/// the merged one at cost `16.746281` against the doubled one's `25.277674`.
/// Merging is what makes the layout depend on the network instead of on the
/// publisher's bookkeeping. It is also the *longer* search of the two, 6 executed
/// sweeps against 3, which is what a map worth more sweeps looks like rather than
/// a saving. (Measured at `llk-001` Phase 7's weights; the same comparison ran
/// `112.087766` over 3 against `139.911271` over 5 at the ones before them, so
/// the direction survived a reweight that inverted the sweep counts.)
///
/// Reversed **and** equal, because two routes with the same list forward draw
/// coincident strokes for the same reason. A route can absorb at most one other
/// anyway: if `a == rev(b)` and `a == rev(c)` then `b == c`, which is the equal
/// case.
fn draws_the_same_line(drawn: &[String], candidate: &[String]) -> bool {
    drawn.len() == candidate.len()
        && (drawn == candidate || drawn.iter().eq(candidate.iter().rev()))
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
    use super::{FALLBACK_PALETTE, draws_the_same_line, stated_color, to_schema};
    use crate::feed::{Feed, Route, Stop, StopTime, Trip};
    use crate::{ImportParams, MergedRoute};

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    fn stop(id: &str) -> Stop {
        Stop {
            stop_id: id.to_string(),
            stop_name: None,
            stop_lat: Some(0.0),
            stop_lon: Some(0.0),
            location_type: None,
            parent_station: None,
        }
    }

    /// Colourless on purpose: the palette index is what the merge must not move.
    fn route(id: &str) -> Route {
        Route {
            route_id: id.to_string(),
            route_short_name: None,
            route_long_name: None,
            route_type: 1,
            route_color: None,
        }
    }

    fn trip((route_id, trip_id): (&str, &str)) -> Trip {
        Trip {
            route_id: route_id.to_string(),
            trip_id: trip_id.to_string(),
        }
    }

    fn stop_time((trip_id, stop_id, stop_sequence): (&str, &str, u32)) -> StopTime {
        StopTime {
            trip_id: trip_id.to_string(),
            stop_id: Some(stop_id.to_string()),
            stop_sequence,
        }
    }

    /// The reversed-pair rule, stated here rather than in the fixture feed.
    ///
    /// OQ-5 fixes that feed's properties at Phase 1 precisely so a later phase
    /// does not extend it and move every literal keyed to it, and this rule
    /// arrived at Phase 4. So it is stated where §2.4's colour predicate and
    /// §2.2's collapse rule are also stated cheaply, and BART is the integration
    /// witness.
    #[test]
    fn a_line_already_drawn_absorbs_its_reverse_and_its_twin_and_nothing_else() {
        let drawn = ids(&["A", "B", "C", "D"]);

        assert!(draws_the_same_line(&drawn, &ids(&["D", "C", "B", "A"])));
        assert!(draws_the_same_line(&drawn, &ids(&["A", "B", "C", "D"])));

        // Every station in common and a different order: a genuinely different
        // line through the same stops, and it contributes corridors this one
        // does not.
        assert!(!draws_the_same_line(&drawn, &ids(&["A", "C", "B", "D"])));
        // A sub-path, which is the short-turn case. It shares corridors without
        // being the same line, and merging it would lose the two it lacks.
        assert!(!draws_the_same_line(&drawn, &ids(&["B", "C"])));
        assert!(!draws_the_same_line(&drawn, &ids(&["A", "B", "C", "E"])));
    }

    /// The half of the rule the predicate alone cannot show: which route keeps
    /// the line, and that the absorbed one costs the palette nothing.
    ///
    /// Every route here is colourless, so the fallback index is observable — and
    /// it is the thing that would silently drift if the merge were checked after
    /// the colour rather than before it. The same ordering argument OQ-3 records
    /// for a dropped route.
    #[test]
    fn the_earlier_route_keeps_the_line_and_the_absorbed_one_takes_no_palette_index() {
        let feed = Feed {
            stops: ["A", "B", "C"].map(stop).into_iter().collect(),
            routes: ["R1", "R2", "R3"].map(route).into_iter().collect(),
            trips: [("R1", "t1"), ("R2", "t2"), ("R3", "t3")]
                .map(trip)
                .into_iter()
                .collect(),
            stop_times: [
                ("t1", "A", 1),
                ("t1", "B", 2),
                ("t1", "C", 3),
                // R2 is R1 backwards, the shape a feed publishing each direction
                // as its own route produces.
                ("t2", "C", 1),
                ("t2", "B", 2),
                ("t2", "A", 3),
                ("t3", "A", 1),
                ("t3", "C", 2),
            ]
            .map(stop_time)
            .into_iter()
            .collect(),
        };

        let (schema, report) =
            to_schema(&feed, &ImportParams::default()).expect("the feed imports");

        let lines: Vec<&str> = schema.lines.iter().map(|line| line.id.as_str()).collect();
        assert_eq!(lines, ["R1", "R3"], "the earlier row keeps the line");
        assert_eq!(
            schema.lines[0].stations,
            ids(&["A", "B", "C"]),
            "and keeps its own direction"
        );
        assert_eq!(
            report.routes_merged,
            [MergedRoute {
                route_id: "R2".to_string(),
                into: "R1".to_string(),
            }]
        );
        assert_eq!(report.routes_kept, 3, "merging is not filtering");

        assert_eq!(schema.lines[0].color, FALLBACK_PALETTE[0]);
        assert_eq!(
            schema.lines[1].color, FALLBACK_PALETTE[1],
            "R2 must not have consumed the second palette colour"
        );
    }

    /// The one conversion whose two branches look alike and are not. A gate
    /// assertion covers both ends of it on the fixture; this is the cheaper
    /// statement of the same rule, and it is what makes a failure there readable.
    #[test]
    fn a_stated_colour_survives_and_only_a_malformed_one_falls_back() {
        assert_eq!(stated_color(Some("FFFFFF")).as_deref(), Some("#FFFFFF"));
        // Case is the feed's to choose; nothing here normalises it.
        assert_eq!(stated_color(Some("0057b8")).as_deref(), Some("#0057b8"));

        for absent_or_malformed in [
            None,
            Some(""),
            Some("#0057B8"),
            Some("57B8"),
            Some("GGGGGG"),
        ] {
            assert_eq!(
                stated_color(absent_or_malformed),
                None,
                "{absent_or_malformed:?} must fall back"
            );
        }
    }
}
