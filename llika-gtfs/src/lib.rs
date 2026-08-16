//! Turn a published GTFS feed into a Llika network file.
//!
//! GTFS read as a **topology** format and not as the timetable it actually is:
//! four tables — `stops`, `routes`, `trips`, `stop_times` — become the stations
//! and ordered line lists `llika-core/src/io.rs:InputSchema` is made of.
//! Calendars, frequencies, transfers, shapes and every arrival time are read
//! past.
//!
//! **The importer writes a file; it does not hand a `Network` to the layout.** A
//! real feed produces something you will want to edit — a depot spur nobody
//! rides, a station name in shouting caps, one branch that makes the map worse —
//! and the schematic-map tradition is editorial rather than a projection of a
//! database. An inspectable JSON file between import and layout is where that
//! editing happens, and it keeps a zip decode out from behind the API boundary
//! `llk-001` built so a later desktop app can hold one `Network` across slider
//! drags.
//!
//! Nothing here reaches into the layout or the renderer, and `llika-core` gains
//! no dependency in return.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod convert;
pub mod feed;
pub mod trips;

pub use convert::FALLBACK_PALETTE;
pub use feed::Feed;

use llika_core::InputSchema;

/// What the import is asked to keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportParams {
    /// The `route_type` values to keep.
    ///
    /// **Filtering is mandatory rather than a convenience.** A city feed is
    /// mostly buses; a schematic poster of a bus network is a different design
    /// problem, and `llk-001`'s layout is `O(iterations · V · r² · E²)`, measured
    /// at 72.9 s release on 200 stations. The importer that hands the layout ten
    /// thousand stations is the importer that makes `llika` look broken.
    ///
    /// `u16` rather than `u8` because the Hierarchical Vehicle Type extension
    /// puts metro at `401` and runs to `1700`. Whether the filter learns those
    /// ranges is a Phase 4 question; the flag already lets a user name the number.
    pub route_types: Vec<u16>,
}

impl Default for ImportParams {
    /// Tram, streetcar and light rail (`0`) plus subway and metro (`1`) — OQ-2.
    ///
    /// Metro alone is the narrowest reading and draws nothing at all for a city
    /// whose system is trams; adding rail (`2`) pulls in regional networks that
    /// sprawl past the poster form and past the envelope `llk-001`'s OQ-9
    /// measured.
    fn default() -> Self {
        Self {
            route_types: vec![0, 1],
        }
    }
}

/// What the import kept, and what it did not.
///
/// Import is lossy by design — routes are filtered out and, from Phase 2,
/// platforms are merged and some routes dropped. Silence about that is how
/// someone concludes the tool lost a line. The binary prints the summary line;
/// the library returns this, so a later desktop app can show the same thing in a
/// panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub routes_seen: usize,
    pub routes_kept: usize,
    pub stops_seen: usize,
    pub stations_emitted: usize,
}

/// Everything that can stop an import.
///
/// Separate from `llika-core/src/io.rs:InputError`, which is about a network
/// file that already exists. The two conditions below are the ones a malformed
/// feed reaches this crate with, and both are hard errors rather than skips: a
/// skip cascades into an `InputError` one step later, where the message names
/// the wrong problem.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("the feed has no `{table}`")]
    MissingTable { table: String },

    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("cannot read the archive {path}: {source}")]
    Archive {
        path: String,
        source: zip::result::ZipError,
    },

    #[error("{table} is not readable as CSV: {source}")]
    Csv {
        table: String,
        source: csv::Error,
    },

    #[error("route `{route}` stops at `{stop_id}`, which stops.txt does not define as a stop or a station")]
    UnknownStop { route: String, stop_id: String },

    #[error("stop `{stop_id}` has no coordinates")]
    MissingCoordinates { stop_id: String },
}

/// Read a feed — a `.zip` or an unpacked directory — and convert it.
pub fn import(
    path: &Path,
    params: &ImportParams,
) -> Result<(InputSchema, ImportReport), ImportError> {
    let feed = Feed::read(path)?;
    convert::to_schema(&feed, params)
}

/// The written form of a network file.
///
/// Pretty-printed with a trailing newline: §1.1 makes the intermediate file
/// hand-editable on purpose, and it is the one place a person overrides what the
/// feed said. It lives here rather than in the binary so the exit gate can
/// assert the bytes the binary writes without shelling out for them.
pub fn to_json(schema: &InputSchema) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(schema)?;
    json.push('\n');
    Ok(json)
}
