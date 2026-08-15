//! Throwaway.

use llika_core::{LayoutParams, Network, parse_input, run_layout, total_cost};

fn main() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sample_network.json"
    ))
    .unwrap();
    let input = parse_input(&text).unwrap();
    let network = Network::from_input(&input).unwrap();

    let base = LayoutParams {
        cluster_moves: false,
        ..LayoutParams::default()
    };
    let params = LayoutParams::default();
    let a = run_layout(&network, &base);
    let b = run_layout(&network, &params);

    println!(
        "baseline t = {:.6}   clusters t = {:.6}",
        total_cost(&network, a.positions(), a.target_edge_cells(), &params),
        total_cost(&network, b.positions(), b.target_edge_cells(), &params),
    );
    let mut moved = 0;
    for (s, (p, q)) in network
        .stations()
        .iter()
        .zip(a.positions().iter().zip(b.positions()))
    {
        if p != q {
            moved += 1;
            println!("  {:<11} ({},{}) -> ({},{})", s.id, p.i, p.j, q.i, q.j);
        }
    }
    println!("moved {moved}");

    for iters in [1u32, 2, 3, 10, 200] {
        let l = run_layout(
            &network,
            &LayoutParams {
                iterations: iters,
                ..LayoutParams::default()
            },
        );
        println!(
            "iterations {iters:>3}: same as 200 = {}",
            l.positions() == b.positions()
        );
    }
    for r in [1u32, 2, 3, 5, 8] {
        let l = run_layout(
            &network,
            &LayoutParams {
                initial_radius: r,
                ..LayoutParams::default()
            },
        );
        println!(
            "radius {r}: same = {}  t = {:.6}",
            l.positions() == b.positions(),
            total_cost(&network, l.positions(), l.target_edge_cells(), &params)
        );
    }
}
