//! The genome: a list of brain-wiring genes (biosim4-style connections)
//! plus a handful of body genes. Reproduction copies it with errors —
//! point mutations, occasional connection add/remove, gene duplication.
//! Nothing here knows what a "good" genome is; the world decides.

use crate::brain::{N_HIDDEN, N_OUTPUTS, N_SENSES};
use crate::config::Config;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// One wiring gene: source (sense or hidden neuron), sink (hidden or output
/// neuron), weight. Duplicate (source, sink) pairs are legal and their
/// weights simply sum — which is what makes gene duplication meaningful.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub from_hidden: bool,
    pub from: u8,
    pub to_output: bool,
    pub to: u8,
    pub weight: f32,
}

impl Connection {
    pub fn random(rng: &mut Rng) -> Self {
        // Bias toward direct sense→output wiring: it is what a founder can
        // actually use before hidden-layer structure has evolved.
        let from_hidden = rng.chance(0.2);
        let to_output = rng.chance(0.7);
        Self {
            from_hidden,
            from: (if from_hidden {
                rng.below(N_HIDDEN)
            } else {
                rng.below(N_SENSES)
            }) as u8,
            to_output,
            to: (if to_output {
                rng.below(N_OUTPUTS)
            } else {
                rng.below(N_HIDDEN)
            }) as u8,
            weight: rng.range_f32(-2.0, 2.0),
        }
    }

    pub fn sort_key(&self) -> (bool, u8, bool, u8) {
        (self.from_hidden, self.from, self.to_output, self.to)
    }
}

/// Body genes. Each trades off against the others through the cost model in
/// `BodyCfg` — none is free, none is best.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Body {
    pub size: f32,
    pub max_speed: f32,
    pub metab: f32,
    pub max_age: f32,
    /// Neutral marker with no physics: lineages become visible as color.
    pub hue: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genome {
    pub connections: Vec<Connection>,
    pub body: Body,
}

impl Genome {
    pub fn random(rng: &mut Rng, cfg: &Config) -> Self {
        let b = &cfg.body;
        let connections = (0..cfg.mutation.initial_connections)
            .map(|_| Connection::random(rng))
            .collect();
        let body = Body {
            size: rng.range_f32(b.size_min, b.size_max),
            max_speed: rng.range_f32(b.speed_min, b.speed_max),
            metab: rng.range_f32(b.metab_min, b.metab_max),
            max_age: rng.range_f32(b.age_min, b.age_max),
            hue: rng.f32(),
        };
        Self { connections, body }
    }

    /// A copy with inheritance errors. RNG draws happen in a fixed order;
    /// this function is the only place heredity errs.
    pub fn mutated(&self, rng: &mut Rng, cfg: &Config) -> Genome {
        let m = &cfg.mutation;
        let mut g = self.clone();

        for c in &mut g.connections {
            if rng.chance(m.weight_p) {
                c.weight =
                    (c.weight + rng.gaussian() * m.weight_sigma).clamp(-m.weight_max, m.weight_max);
            }
            if rng.chance(m.rewire_p) {
                if rng.chance(0.5) {
                    c.from_hidden = rng.chance(0.2);
                    c.from = (if c.from_hidden {
                        rng.below(N_HIDDEN)
                    } else {
                        rng.below(N_SENSES)
                    }) as u8;
                } else {
                    c.to_output = rng.chance(0.7);
                    c.to = (if c.to_output {
                        rng.below(N_OUTPUTS)
                    } else {
                        rng.below(N_HIDDEN)
                    }) as u8;
                }
            }
        }
        if rng.chance(m.add_p) && g.connections.len() < m.genome_cap {
            g.connections.push(Connection::random(rng));
        }
        if rng.chance(m.remove_p) && g.connections.len() > 1 {
            let i = rng.below(g.connections.len());
            g.connections.remove(i);
        }
        if rng.chance(m.duplicate_p)
            && !g.connections.is_empty()
            && g.connections.len() < m.genome_cap
        {
            let i = rng.below(g.connections.len());
            let c = g.connections[i];
            g.connections.push(c);
        }

        let b = &cfg.body;
        let bd = &mut g.body;
        if rng.chance(m.body_p) {
            bd.size = (bd.size + rng.gaussian() * m.body_sigma_frac * (b.size_max - b.size_min))
                .clamp(b.size_min, b.size_max);
        }
        if rng.chance(m.body_p) {
            bd.max_speed = (bd.max_speed
                + rng.gaussian() * m.body_sigma_frac * (b.speed_max - b.speed_min))
                .clamp(b.speed_min, b.speed_max);
        }
        if rng.chance(m.body_p) {
            bd.metab = (bd.metab
                + rng.gaussian() * m.body_sigma_frac * (b.metab_max - b.metab_min))
                .clamp(b.metab_min, b.metab_max);
        }
        if rng.chance(m.body_p) {
            bd.max_age = (bd.max_age
                + rng.gaussian() * m.body_sigma_frac * (b.age_max - b.age_min))
                .clamp(b.age_min, b.age_max);
        }
        if rng.chance(m.body_p) {
            // Hue is circular: it wraps rather than clamps, so it drifts freely.
            bd.hue = (bd.hue + rng.gaussian() * m.body_sigma_frac).rem_euclid(1.0);
        }
        g
    }
}

/// Connections sorted for the merge in [`genome_distance`].
pub fn sorted_connections(g: &Genome) -> Vec<Connection> {
    let mut v = g.connections.clone();
    v.sort_by(|a, b| {
        a.sort_key()
            .cmp(&b.sort_key())
            .then(a.weight.total_cmp(&b.weight))
    });
    v
}

/// Normalized distance between two genomes in [0, ~1]: mean of a body-gene
/// distance and a connection-set distance (matched wiring compares weights,
/// unmatched wiring counts as a full difference).
pub fn genome_distance(
    a: &Genome,
    a_sorted: &[Connection],
    b: &Genome,
    b_sorted: &[Connection],
    cfg: &Config,
) -> f32 {
    let bb = &cfg.body;
    let nd = |x: f32, y: f32, lo: f32, hi: f32| (x - y).abs() / (hi - lo);
    let hue_d = {
        let d = (a.body.hue - b.body.hue).abs();
        d.min(1.0 - d) * 2.0
    };
    let body = (nd(a.body.size, b.body.size, bb.size_min, bb.size_max)
        + nd(
            a.body.max_speed,
            b.body.max_speed,
            bb.speed_min,
            bb.speed_max,
        )
        + nd(a.body.metab, b.body.metab, bb.metab_min, bb.metab_max)
        + nd(a.body.max_age, b.body.max_age, bb.age_min, bb.age_max)
        + hue_d)
        / 5.0;

    let (mut i, mut j) = (0usize, 0usize);
    let mut matched = 0.0f32;
    let mut pairs = 0u32;
    let mut unmatched = 0u32;
    while i < a_sorted.len() && j < b_sorted.len() {
        match a_sorted[i].sort_key().cmp(&b_sorted[j].sort_key()) {
            core::cmp::Ordering::Equal => {
                matched += (a_sorted[i].weight - b_sorted[j].weight).abs()
                    / (2.0 * cfg.mutation.weight_max);
                pairs += 1;
                i += 1;
                j += 1;
            }
            core::cmp::Ordering::Less => {
                unmatched += 1;
                i += 1;
            }
            core::cmp::Ordering::Greater => {
                unmatched += 1;
                j += 1;
            }
        }
    }
    unmatched += (a_sorted.len() - i) as u32 + (b_sorted.len() - j) as u32;
    let union = pairs + unmatched;
    let conn = if union == 0 {
        0.0
    } else {
        (matched + unmatched as f32) / union as f32
    };
    0.5 * (body + conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_respects_ranges_and_cap() {
        let cfg = Config::default();
        let mut rng = Rng::from_seed(9);
        let mut g = Genome::random(&mut rng, &cfg);
        for _ in 0..2000 {
            g = g.mutated(&mut rng, &cfg);
            let b = &g.body;
            let c = &cfg.body;
            assert!(b.size >= c.size_min && b.size <= c.size_max);
            assert!(b.max_speed >= c.speed_min && b.max_speed <= c.speed_max);
            assert!(b.metab >= c.metab_min && b.metab <= c.metab_max);
            assert!(b.max_age >= c.age_min && b.max_age <= c.age_max);
            assert!(b.hue >= 0.0 && b.hue < 1.0);
            assert!(!g.connections.is_empty());
            assert!(g.connections.len() <= cfg.mutation.genome_cap);
            for conn in &g.connections {
                assert!(conn.weight.abs() <= cfg.mutation.weight_max);
                let src_pool = if conn.from_hidden { N_HIDDEN } else { N_SENSES };
                let dst_pool = if conn.to_output { N_OUTPUTS } else { N_HIDDEN };
                assert!((conn.from as usize) < src_pool);
                assert!((conn.to as usize) < dst_pool);
            }
        }
    }

    #[test]
    fn distance_is_zero_for_identical_and_positive_for_different() {
        let cfg = Config::default();
        let mut rng = Rng::from_seed(4);
        let a = Genome::random(&mut rng, &cfg);
        let b = Genome::random(&mut rng, &cfg);
        let asc = sorted_connections(&a);
        let bsc = sorted_connections(&b);
        assert_eq!(genome_distance(&a, &asc, &a, &asc, &cfg), 0.0);
        assert!(genome_distance(&a, &asc, &b, &bsc, &cfg) > 0.0);
    }
}
