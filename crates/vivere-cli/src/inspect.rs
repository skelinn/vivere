//! The genome census: what did evolution actually build? Read-only over a
//! decoded world — no RNG, no stepping — so inspection can never perturb
//! anything. Distinguishes live wiring from junk (the mutation ratchet adds
//! connections faster than it removes them, so length alone proves
//! nothing): a gene is *weak* if its weight is negligible, *dead* if it
//! feeds or reads a hidden neuron that nothing completes (in-degree-0
//! sources emit a constant tanh(0)=0; out-degree-0 sinks are never read),
//! and duplicate genes on one edge are counted as *stacking* (live gain
//! beyond the ±weight_max clamp, not junk).

use std::collections::HashMap;
use std::fmt::Write as _;
use vivere_core::World;
use vivere_core::brain::{N_HIDDEN, N_OUTPUTS, N_SENSES, OUTPUT_NAMES, SENSE_NAMES};

const WEAK_EPS: f32 = 0.01;

#[derive(Default)]
struct Census {
    organisms: usize,
    genes: u64,
    distinct_edges: u64,
    dup_extras: u64,
    weak: u64,
    dead: u64,
    live: u64,
    stacked_edges: u64,
    stacked_over_clamp: u64,
    weights_at_clamp: u64,
    len_min: usize,
    len_max: usize,
    len_sum: f64,
    len_sq_sum: f64,
    hidden_alive_sum: f64,
    hidden_alive_by_neuron: [u64; N_HIDDEN],
    sense_genes: [u64; N_SENSES],
    sense_users: [u64; N_SENSES],
    sense_weight: [f64; N_SENSES],
    output_genes: [u64; N_OUTPUTS],
    output_users: [u64; N_OUTPUTS],
    output_weight: [f64; N_OUTPUTS],
    len_hist: [u64; 9],
}

fn hist_bucket(n: usize) -> usize {
    // 0-7, 8-15, 16-31, 32-63, 64-127, 128-255, 256-511, 512-1023, 1024+
    match n {
        0..=7 => 0,
        8..=15 => 1,
        16..=31 => 2,
        32..=63 => 3,
        64..=127 => 4,
        128..=255 => 5,
        256..=511 => 6,
        512..=1023 => 7,
        _ => 8,
    }
}

const HIST_LABELS: [&str; 9] = [
    "0-7", "8-15", "16-31", "32-63", "64-127", "128-255", "256-511", "512-1023", "1024+",
];

fn census(world: &World) -> Census {
    let mut c = Census {
        len_min: usize::MAX,
        ..Default::default()
    };
    c.organisms = world.organisms.len();

    for o in &world.organisms {
        let conns = &o.genome.connections;
        let n = conns.len();
        c.genes += n as u64;
        c.len_min = c.len_min.min(n);
        c.len_max = c.len_max.max(n);
        c.len_sum += n as f64;
        c.len_sq_sum += (n * n) as f64;
        c.len_hist[hist_bucket(n)] += 1;

        let mut in_deg = [0u32; N_HIDDEN];
        let mut out_deg = [0u32; N_HIDDEN];
        for g in conns {
            if !g.to_output {
                in_deg[g.to as usize] += 1;
            }
            if g.from_hidden {
                out_deg[g.from as usize] += 1;
            }
        }
        let mut alive_here = 0u32;
        for k in 0..N_HIDDEN {
            if in_deg[k] >= 1 && out_deg[k] >= 1 {
                c.hidden_alive_by_neuron[k] += 1;
                alive_here += 1;
            }
        }
        c.hidden_alive_sum += f64::from(alive_here);

        let mut edges: HashMap<(bool, u8, bool, u8), (u32, f32)> = HashMap::new();
        let mut sense_seen = [false; N_SENSES];
        let mut output_seen = [false; N_OUTPUTS];
        for g in conns {
            let e = edges
                .entry((g.from_hidden, g.from, g.to_output, g.to))
                .or_insert((0, 0.0));
            e.0 += 1;
            e.1 += g.weight;

            if g.weight.abs() > 3.9 {
                c.weights_at_clamp += 1;
            }
            let weak = g.weight.abs() < WEAK_EPS;
            let dead_src = g.from_hidden && in_deg[g.from as usize] == 0;
            let dead_sink = !g.to_output && out_deg[g.to as usize] == 0;
            if weak {
                c.weak += 1;
            } else if dead_src || dead_sink {
                c.dead += 1;
            } else {
                c.live += 1;
                if !g.from_hidden {
                    c.sense_genes[g.from as usize] += 1;
                    c.sense_weight[g.from as usize] += f64::from(g.weight.abs());
                    sense_seen[g.from as usize] = true;
                }
                if g.to_output {
                    c.output_genes[g.to as usize] += 1;
                    c.output_weight[g.to as usize] += f64::from(g.weight.abs());
                    output_seen[g.to as usize] = true;
                }
            }
        }
        c.distinct_edges += edges.len() as u64;
        for (_, (count, sum_w)) in edges {
            if count >= 2 {
                c.stacked_edges += 1;
                c.dup_extras += u64::from(count - 1);
                if sum_w.abs() > 4.0 {
                    c.stacked_over_clamp += 1;
                }
            }
        }
        for (s, seen) in sense_seen.iter().enumerate() {
            if *seen {
                c.sense_users[s] += 1;
            }
        }
        for (s, seen) in output_seen.iter().enumerate() {
            if *seen {
                c.output_users[s] += 1;
            }
        }
    }
    if c.organisms == 0 {
        c.len_min = 0;
    }
    c
}

/// `key value` lines for fixtures and scripts.
pub fn summary(world: &World) -> String {
    let c = census(world);
    let mut s = String::new();
    let _ = writeln!(s, "tick {}", world.tick);
    let _ = writeln!(s, "population {}", world.organisms.len());
    let _ = writeln!(s, "food {}", world.food.len());
    let _ = writeln!(s, "detritus {}", world.detritus.len());
    let _ = writeln!(s, "births {}", world.births);
    let _ = writeln!(s, "deaths_starve {}", world.deaths_starve);
    let _ = writeln!(s, "deaths_age {}", world.deaths_age);
    let _ = writeln!(s, "deaths_predation {}", world.deaths_predation);
    let _ = writeln!(s, "bites {}", world.bites);
    let _ = writeln!(s, "next_id {}", world.next_id);
    let _ = writeln!(s, "injected {:.6}", world.ledger.injected);
    let _ = writeln!(s, "radiated {:.6}", world.ledger.radiated);
    let _ = writeln!(s, "world_energy {:.6}", world.world_energy());
    let _ = writeln!(s, "genes {}", c.genes);
    let _ = writeln!(
        s,
        "mean_genome_len {:.4}",
        c.len_sum / c.organisms.max(1) as f64
    );
    s
}

/// The full human-readable census.
pub fn report(world: &World) -> String {
    let c = census(world);
    let n = c.organisms.max(1) as f64;
    let genes = c.genes.max(1) as f64;
    let mean_len = c.len_sum / n;
    let var = (c.len_sq_sum / n - mean_len * mean_len).max(0.0);

    let mut s = String::new();
    let _ = writeln!(
        s,
        "world: tick {}  population {}  food {}  detritus {}  contact {}",
        world.tick,
        c.organisms,
        world.food.len(),
        world.detritus.len(),
        if world.cfg.contact.enabled {
            "on"
        } else {
            "off"
        },
    );
    let _ = writeln!(
        s,
        "ledger: injected {:.1}  radiated {:.1}  drift {:.3e}",
        world.ledger.injected,
        world.ledger.radiated,
        world.conservation_error(),
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "genome length: mean {:.1}  std {:.1}  min {}  max {}  (cap {})",
        mean_len,
        var.sqrt(),
        c.len_min,
        c.len_max,
        world.cfg.mutation.genome_cap,
    );
    for (label, count) in HIST_LABELS.iter().zip(c.len_hist) {
        if count > 0 {
            let _ = writeln!(
                s,
                "  {label:>9}  {count:>6}  {}",
                bar(count, c.organisms as u64)
            );
        }
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "gene census ({} genes, {} distinct edges):",
        c.genes, c.distinct_edges
    );
    let _ = writeln!(
        s,
        "  live {:>7}  ({:.1}%)",
        c.live,
        100.0 * c.live as f64 / genes
    );
    let _ = writeln!(
        s,
        "  weak {:>7}  ({:.1}%)   |w| < {WEAK_EPS}",
        c.weak,
        100.0 * c.weak as f64 / genes
    );
    let _ = writeln!(
        s,
        "  dead {:>7}  ({:.1}%)   feeds/reads a hidden neuron nothing completes",
        c.dead,
        100.0 * c.dead as f64 / genes
    );
    let _ = writeln!(
        s,
        "  duplicate extras {} on {} stacked edges ({} exceed the ±4 clamp when summed)",
        c.dup_extras, c.stacked_edges, c.stacked_over_clamp
    );
    let _ = writeln!(
        s,
        "  weights at clamp (|w| > 3.9): {} ({:.1}%)",
        c.weights_at_clamp,
        100.0 * c.weights_at_clamp as f64 / genes
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "hidden pool: mean {:.2}/{} neurons alive (in-degree ≥1 AND out-degree ≥1)",
        c.hidden_alive_sum / n,
        N_HIDDEN
    );
    for k in 0..N_HIDDEN {
        let _ = writeln!(
            s,
            "  h{k}: alive in {:>5.1}% of organisms",
            100.0 * c.hidden_alive_by_neuron[k] as f64 / n
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "sense wiring (live genes only):");
    for (i, name) in SENSE_NAMES.iter().enumerate() {
        let users = 100.0 * c.sense_users[i] as f64 / n;
        let per_org = c.sense_genes[i] as f64 / n;
        let mean_w = c.sense_weight[i] / c.sense_genes[i].max(1) as f64;
        let _ = writeln!(
            s,
            "  {name:>9}  wired by {users:>5.1}%  {per_org:>5.2} genes/org  mean|w| {mean_w:.2}"
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "output wiring (live genes only):");
    for (i, name) in OUTPUT_NAMES.iter().enumerate() {
        let users = 100.0 * c.output_users[i] as f64 / n;
        let per_org = c.output_genes[i] as f64 / n;
        let mean_w = c.output_weight[i] / c.output_genes[i].max(1) as f64;
        let _ = writeln!(
            s,
            "  {name:>9}  wired by {users:>5.1}%  {per_org:>5.2} genes/org  mean|w| {mean_w:.2}"
        );
    }
    s
}

fn bar(count: u64, total: u64) -> String {
    let width = (count as f64 / total.max(1) as f64 * 40.0).round() as usize;
    "#".repeat(width.max(1))
}
