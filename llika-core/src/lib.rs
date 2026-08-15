//! Llika's core: a JSON metro network in, one schematic SVG out.
//!
//! The three steps are public in their own right and not only reachable through
//! [`build_schematic_svg`]. That is what lets a future desktop app parse and
//! project a loaded file **once**, hold the [`Network`] in memory, and on every
//! slider drag call only [`run_layout`] and [`render_to_string`] with new
//! parameters.

pub mod geometry;
pub mod grid;
pub mod io;
pub mod layout;
pub mod model;
pub mod projection;
pub mod render;

pub use io::{InputError, InputSchema, parse_input};
pub use layout::{LayoutParams, SchematicLayout, run_layout};
pub use model::{Line, Network, Station};
pub use render::{RenderParams, render_to_string};

/// Parse, lay out and render in one call.
pub fn build_schematic_svg(
    input: &InputSchema,
    layout_params: &LayoutParams,
    render_params: &RenderParams,
) -> Result<String, InputError> {
    let network = Network::from_input(input)?;
    let layout = run_layout(&network, layout_params);
    Ok(render_to_string(&network, &layout, render_params))
}
