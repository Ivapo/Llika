//! The four tables of the spec's §2.1, read from a `.zip` or a directory.
//!
//! **Every column that is not unconditionally `Required` in GTFS is an `Option`,
//! and every one of them also carries `#[serde(default)]`.** The two do
//! different jobs and both are needed: the `Option` covers an empty *cell*, which
//! `csv` hands to serde as `None`; `#[serde(default)]` covers an absent
//! *column*, which serde otherwise reports as a missing field and which kills the
//! parse on a feed that simply never wrote `parent_station`. The reflex parser
//! types these bare and dies on the first real feed — `stop_lat`/`stop_lon` are
//! Conditionally Required and absent on entrances, and `stop_times.stop_id` is
//! forbidden outright on a GTFS-Flex row.
//!
//! Every table is a `Vec` in **file row order**. §2.5 makes input order the
//! iteration order here as it is one crate over, and a `HashMap` walked to
//! produce the `stations` array would vary the output file per process.
//!
//! **`stop_times` is a filtered subset of that order, and the other three are
//! whole.** A city's `stop_times.txt` runs to hundreds of megabytes — 373 MB and
//! ~6.2 M rows on Chicago's feed, against 7.7 MB for the other three tables put
//! together — for a topology importer that keeps the rows of one trip per route.
//! So that one table streams past and only the rows of a kept route's trips are
//! retained. Row order is untouched by it: a filter preserves order, and every
//! downstream guarantee still rests on that.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{ImportError, ImportParams};

pub const STOPS: &str = "stops.txt";
pub const ROUTES: &str = "routes.txt";
pub const TRIPS: &str = "trips.txt";
pub const STOP_TIMES: &str = "stop_times.txt";

/// One `stops.txt` row.
#[derive(Debug, Clone, Deserialize)]
pub struct Stop {
    pub stop_id: String,
    #[serde(default)]
    pub stop_name: Option<String>,
    #[serde(default)]
    pub stop_lat: Option<f64>,
    #[serde(default)]
    pub stop_lon: Option<f64>,
    #[serde(default)]
    pub location_type: Option<u8>,
    /// Read but unused until Phase 2, which is where platforms collapse.
    #[serde(default)]
    pub parent_station: Option<String>,
}

impl Stop {
    /// Whether §2.1 reads this row at all.
    ///
    /// `0` (or empty) is a stop or platform and `1` a station; `2`, `3` and `4`
    /// are an entrance, a generic node and a boarding area, and are exactly the
    /// rows that may carry no coordinates while requiring a `parent_station`.
    /// Reading them would make every downstream station carry an `Option`
    /// position for the sake of rows that can never be drawn.
    pub fn is_stop_or_station(&self) -> bool {
        matches!(self.location_type, None | Some(0) | Some(1))
    }

    /// The display name, falling back to the id so the written file stays
    /// readable by a person editing it.
    pub fn name(&self) -> &str {
        match self.stop_name.as_deref() {
            Some(name) if !name.is_empty() => name,
            _ => &self.stop_id,
        }
    }
}

/// One `routes.txt` row.
#[derive(Debug, Clone, Deserialize)]
pub struct Route {
    pub route_id: String,
    /// Both name columns are Conditionally Required — the standard says at least
    /// one must be defined — so [`Route::name`]'s third rung is stricter than
    /// GTFS requires and exists because a feed breaking that rule should still
    /// import.
    #[serde(default)]
    pub route_short_name: Option<String>,
    #[serde(default)]
    pub route_long_name: Option<String>,
    pub route_type: u16,
    #[serde(default)]
    pub route_color: Option<String>,
}

impl Route {
    /// `route_short_name`, then `route_long_name`, then `route_id` — §2.4.
    pub fn name(&self) -> &str {
        for candidate in [&self.route_short_name, &self.route_long_name] {
            if let Some(name) = candidate.as_deref()
                && !name.is_empty()
            {
                return name;
            }
        }
        &self.route_id
    }
}

/// One `trips.txt` row. `direction_id` is deliberately absent: §2.1 names it as
/// a column this spec would have to add if OQ-1 resolves that way, and Phase 3
/// owns that.
#[derive(Debug, Clone, Deserialize)]
pub struct Trip {
    pub route_id: String,
    pub trip_id: String,
}

/// One `stop_times.txt` row. The arrival and departure times are read past —
/// this is a topology importer and GTFS's timetable is out of scope.
#[derive(Debug, Clone, Deserialize)]
pub struct StopTime {
    pub trip_id: String,
    /// Conditionally Required: forbidden on a GTFS-Flex row carrying
    /// `location_group_id` or `location_id`. Whether such a row is skipped or is
    /// an error is Phase 4's open item; until then it contributes no stop and so
    /// is passed over.
    #[serde(default)]
    pub stop_id: Option<String>,
    /// Required to increase along a trip but **not** to be consecutive — feeds
    /// using 10, 20, 30 are common — and the file is not required to store the
    /// rows in that order. So this value is the sort key, never the row order and
    /// never a difference between values.
    pub stop_sequence: u32,
}

/// The four tables, in file row order — the first three whole, and
/// `stop_times` filtered to the trips the import will use.
#[derive(Debug, Clone)]
pub struct Feed {
    pub stops: Vec<Stop>,
    pub routes: Vec<Route>,
    pub trips: Vec<Trip>,
    /// The rows of trips whose route survives [`ImportParams::keeps`], and no
    /// others. See [`Feed::read`] for why the subset is the table.
    pub stop_times: Vec<StopTime>,
}

impl Feed {
    /// Read a feed from a `.zip` or from an unpacked directory.
    ///
    /// Both, because every published feed is a zip and requiring a manual unzip
    /// in the phase that promises a real network undercuts the point. The two
    /// routes must agree byte for byte, which the exit gate asserts.
    ///
    /// **`params` is here because `stop_times` is filtered as it streams**, and
    /// filtering needs the answer before the stream runs. The four tables are
    /// read in a fixed order with `stop_times` last, so the kept routes and then
    /// the kept trips are both known by the time it starts — the ordering is
    /// what makes one pass enough.
    ///
    /// **The params that read a feed are the params that must convert it.**
    /// Nothing in the types enforces that, and a `convert::to_schema` called
    /// with a *wider* set than the one here would find a newly kept route's rows
    /// already discarded and truncate its line rather than fail.
    /// `lib.rs:import` is the one caller and passes the same value to both.
    pub fn read(path: &Path, params: &ImportParams) -> Result<Self, ImportError> {
        let mut source = Source::open(path)?;
        // Read in a fixed order, so an error always names the same table first
        // and so the filter below has its inputs.
        let stops = source.table(STOPS)?;
        let routes: Vec<Route> = source.table(ROUTES)?;
        let trips: Vec<Trip> = source.table(TRIPS)?;

        // Scoped: the two sets borrow `routes` and `trips`, whose drop glue
        // holds the borrow to the end of the enclosing block, and both vectors
        // are moved into `Self` below.
        let stop_times = {
            let kept_routes: HashSet<&str> = routes
                .iter()
                .filter(|route| params.keeps(route.route_type))
                .map(|route| route.route_id.as_str())
                .collect();
            let kept_trips: HashSet<&str> = trips
                .iter()
                .filter(|trip| kept_routes.contains(trip.route_id.as_str()))
                .map(|trip| trip.trip_id.as_str())
                .collect();
            source.retained(STOP_TIMES, |row: &StopTime| {
                kept_trips.contains(row.trip_id.as_str())
            })?
        };

        Ok(Self {
            stops,
            routes,
            trips,
            stop_times,
        })
    }
}

/// Where the four tables come from.
enum Source {
    Directory(PathBuf),
    /// Boxed: a `ZipArchive` is large, and an unboxed variant would make every
    /// `Source` that size.
    Archive(Box<zip::ZipArchive<BufReader<File>>>),
}

impl Source {
    fn open(path: &Path) -> Result<Self, ImportError> {
        if path.is_dir() {
            return Ok(Self::Directory(path.to_path_buf()));
        }
        let file = File::open(path).map_err(|source| ImportError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let archive =
            zip::ZipArchive::new(BufReader::new(file)).map_err(|source| ImportError::Archive {
                path: path.display().to_string(),
                source,
            })?;
        Ok(Self::Archive(Box::new(archive)))
    }

    /// One table, whole.
    fn table<T: DeserializeOwned>(&mut self, name: &str) -> Result<Vec<T>, ImportError> {
        self.retained(name, |_| true)
    }

    /// The rows of one table that `keep` accepts.
    ///
    /// Rows are deserialized one at a time from a reader that is never
    /// collected, so the memory cost is the rows kept rather than the size of
    /// the file: a 373 MB `stop_times.txt` whose kept trips are 0.9% of it costs
    /// 0.9% of it. Every row is still parsed, which is unavoidable — the
    /// `trip_id` deciding the row is a field of the row.
    fn retained<T: DeserializeOwned>(
        &mut self,
        name: &str,
        keep: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>, ImportError> {
        let reader = self.entry(name)?;
        let mut records = Vec::new();
        for record in csv::Reader::from_reader(reader).into_deserialize() {
            let record = record.map_err(|source| ImportError::Csv {
                table: name.to_string(),
                source,
            })?;
            if keep(&record) {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// One table's bytes, as a reader.
    ///
    /// **The archive entry streams.** `by_name` borrows the archive, which
    /// forbids holding two entries open at once — and nothing here does, since
    /// the tables are read one after another. `zip::ZipFile` is a `Read` and
    /// `csv::Reader` consumes one incrementally, so the bytes never all exist at
    /// once. A read failing part-way now surfaces as [`ImportError::Csv`] naming
    /// the table rather than [`ImportError::Io`] naming the entry, since the
    /// read happens inside `csv`.
    fn entry(&mut self, name: &str) -> Result<Box<dyn Read + '_>, ImportError> {
        match self {
            Self::Directory(dir) => {
                let path = dir.join(name);
                let file = File::open(&path).map_err(|source| {
                    if source.kind() == std::io::ErrorKind::NotFound {
                        ImportError::MissingTable {
                            table: name.to_string(),
                        }
                    } else {
                        ImportError::Io {
                            path: path.display().to_string(),
                            source,
                        }
                    }
                })?;
                Ok(Box::new(BufReader::new(file)))
            }
            Self::Archive(archive) => {
                let entry = archive
                    .by_name(name)
                    .map_err(|_| ImportError::MissingTable {
                        table: name.to_string(),
                    })?;
                Ok(Box::new(entry))
            }
        }
    }
}
