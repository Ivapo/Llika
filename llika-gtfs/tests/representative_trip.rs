//! Phase 3's exit gate, assertions 1-3. Assertion 4 — cross-process byte
//! stability, still — is `byte_stability.rs`, unchanged from Phase 1 because the
//! property is.
//!
//! What this phase claims is that the line a route draws is the line as
//! **operated**: the pattern the most of its trips run, rather than whichever
//! trip happens to be longest. Phase 1 shipped the longest deliberately, so the
//! question would be settled against a drawn map — so each assertion below
//! carries the control that names what the naive rule produced, and the pair is
//! what makes this read as a change rather than a coincidence.
//!
//! All three read the **shared** fixture, whose properties OQ-5 fixed at Phase 1
//! for exactly this reason. A second feed authored here would be the
//! cheaper-looking answer; extending the shared one now is what moves every
//! Phase 1 and Phase 2 literal keyed to it.
//!
//! One rule this gate cannot reach is in `llika-gtfs/src/trips.rs`'s own tests:
//! that two trips differing only in which platform of a station they serve are
//! **one** pattern. OQ-5 fixed the fixture's properties before Phase 1 and does
//! not fix one for it, and extending the feed to witness it is the churn that
//! question exists to prevent.
//!
//! The fixture's engineered properties, and which assertion consumes each, are
//! documented in `common/mod.rs`.

mod common;

use common::{import_fixture, line_stations};

/// Assertion 1, and the only one that discriminates: `M5`'s longest trip is a
/// six-stop special that one trip runs, and its modal pattern is the four-stop
/// one that three do.
///
/// The control is Phase 1's committed answer. `M5_Q1` is the trip a rule keyed
/// on length picks, and it is the shape of the failure OQ-1 describes — a line
/// running on to two stations no ordinary service reaches.
#[test]
fn a_route_draws_its_modal_pattern_and_not_its_longest_trip() {
    let (schema, _) = import_fixture();

    assert_eq!(line_stations(&schema, "M5"), ["NOR", "HIL", "RIV", "OLD"]);
    // What the longest trip drew, through Phase 2.
    assert_ne!(
        line_stations(&schema, "M5"),
        ["NOR", "HIL", "RIV", "OLD", "CEN", "FAI"],
    );
}

/// Assertion 2: the degenerate case every selection rule has to survive.
///
/// `M1` has exactly one trip, so its one pattern is both the modal and the
/// longest and the two rules agree. That is what makes it the right literal for
/// `import.rs`'s ordered-list assertion, which is keyed to it for this reason
/// and does not move across phases.
#[test]
fn a_route_with_one_trip_still_imports() {
    let (schema, _) = import_fixture();

    assert_eq!(
        line_stations(&schema, "M1"),
        ["WST", "OLD", "CEN", "MKT", "EAST"],
    );
}

/// Assertion 3: the tie-break, on `llk-001` §2.4's grounds — equal-cost
/// candidates are common rather than exotic, and leaving the choice to
/// enumeration order makes the output depend on it.
///
/// `M6` runs two patterns two trips each: `R` (`M6_R1`, `M6_R2`) and `S`
/// (`M6_S1`, `M6_S2`). Neither is modal, so the count decides nothing and the
/// tie-break decides everything: the pattern whose first trip is the earlier
/// `trips.txt` row, which is `R`.
///
/// The `assert_ne!` is the whole assertion's discriminating half — `S` is what
/// a tie resolved the other way, or by enumeration order, would draw.
#[test]
fn a_tie_on_trip_count_goes_to_the_earlier_trips_txt_row() {
    let (schema, _) = import_fixture();

    assert_eq!(line_stations(&schema, "M6"), ["MKT", "QUA", "FAI"]);
    assert_ne!(line_stations(&schema, "M6"), ["FAI", "QUA", "SOU"]);
}
