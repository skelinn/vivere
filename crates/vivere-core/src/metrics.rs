//! Observation. Metrics read the world without touching it: the diversity
//! sample draws from a throwaway RNG derived from (seed, tick), so whether
//! and how often you measure can never change what happens.

use crate::brain::N_HIDDEN;
use crate::genome::{genome_distance, sorted_connections};
use crate::rng::Rng;
use crate::world::World;
use serde::{Deserialize, Serialize};

/// How many organisms the genome-diversity estimate samples.
const DIVERSITY_SAMPLE: usize = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub tick: u64,
    pub population: usize,
    /// Cumulative counters; the CSV writer turns them into per-window counts.
    pub births_total: u64,
    pub deaths_starve_total: u64,
    pub deaths_age_total: u64,
    pub deaths_predation_total: u64,
    pub mean_energy: f64,
    pub max_energy: f64,
    pub mean_genome_len: f64,
    /// Mean pairwise genome distance over a fixed-size sample.
    pub diversity: f64,
    pub mean_size: f64,
    pub mean_speed_gene: f64,
    pub mean_actual_speed: f64,
    pub mean_metab: f64,
    pub mean_max_age: f64,
    pub mean_generation: f64,
    pub food_count: usize,
    pub detritus_count: usize,
    pub world_energy: f64,
    pub injected: f64,
    pub radiated: f64,
    /// world_energy + radiated − injected. Zero when energy is conserved.
    pub drift: f64,
    pub bites_total: u64,
    pub predation_flux_total: f64,
    pub bite_hue_sum_total: f64,
    pub neighbor_hue_sum_total: f64,
    pub neighbor_hue_count_total: u64,
    /// Std dev of ln(size) / ln(max_speed): scale-free spread — a rising
    /// value is the early warning of a population splitting in two.
    pub std_size_log: f64,
    pub std_speed_log: f64,
    /// Genome-length spread and maximum (the cap tripwire: if max ever
    /// exceeds 0.8 × genome_cap, the cap rises next release).
    pub std_genome_len: f64,
    pub max_genome_len: usize,
    /// Mean hidden neurons per organism that are actually alive
    /// (in-degree ≥ 1 AND out-degree ≥ 1 — write-only neurons are dead).
    pub mean_hidden_used: f64,
    /// Pearson correlation between connection count and bite income per
    /// tick of life — the cognition-stratification signal: do bigger
    /// brains belong to better predators?
    pub corr_brain_gain: f64,
    /// Fraction of genes in the diversity sample that are live wiring
    /// (non-negligible weight, not dead-ended on an unused hidden neuron).
    /// Distinguishes minds from mutation-ratchet junk mid-run.
    pub live_conn_frac: f64,
}

impl Metrics {
    pub fn measure(world: &World) -> Metrics {
        let n = world.organisms.len();
        let inv = if n > 0 { 1.0 / n as f64 } else { 0.0 };
        let mean = |f: &dyn Fn(&crate::organism::Organism) -> f64| -> f64 {
            world.organisms.iter().map(f).sum::<f64>() * inv
        };

        let std_log = |f: &dyn Fn(&crate::organism::Organism) -> f64| -> f64 {
            if n < 2 {
                return 0.0;
            }
            let mean = world.organisms.iter().map(|o| libm::log(f(o))).sum::<f64>() * inv;
            let var = world
                .organisms
                .iter()
                .map(|o| {
                    let d = libm::log(f(o)) - mean;
                    d * d
                })
                .sum::<f64>()
                * inv;
            var.sqrt()
        };

        let (diversity, live_conn_frac) = diversity_and_live(world);

        Metrics {
            tick: world.tick,
            population: n,
            births_total: world.births,
            deaths_starve_total: world.deaths_starve,
            deaths_age_total: world.deaths_age,
            deaths_predation_total: world.deaths_predation,
            mean_energy: mean(&|o| o.energy),
            max_energy: world.organisms.iter().map(|o| o.energy).fold(0.0, f64::max),
            mean_genome_len: mean(&|o| o.genome.connections.len() as f64),
            diversity,
            mean_size: mean(&|o| o.genome.body.size as f64),
            mean_speed_gene: mean(&|o| o.genome.body.max_speed as f64),
            mean_actual_speed: mean(&|o| o.last_speed as f64),
            mean_metab: mean(&|o| o.genome.body.metab as f64),
            mean_max_age: mean(&|o| o.genome.body.max_age as f64),
            mean_generation: mean(&|o| o.generation as f64),
            food_count: world.food.len(),
            detritus_count: world.detritus.len(),
            world_energy: world.world_energy(),
            injected: world.ledger.injected,
            radiated: world.ledger.radiated,
            drift: world.conservation_error(),
            bites_total: world.bites,
            predation_flux_total: world.predation_flux,
            bite_hue_sum_total: world.bite_hue_sum,
            neighbor_hue_sum_total: world.neighbor_hue_sum,
            neighbor_hue_count_total: world.neighbor_hue_count,
            std_size_log: std_log(&|o| o.genome.body.size as f64),
            std_speed_log: std_log(&|o| o.genome.body.max_speed as f64),
            std_genome_len: {
                let mean = world
                    .organisms
                    .iter()
                    .map(|o| o.genome.connections.len() as f64)
                    .sum::<f64>()
                    * inv;
                (world
                    .organisms
                    .iter()
                    .map(|o| {
                        let d = o.genome.connections.len() as f64 - mean;
                        d * d
                    })
                    .sum::<f64>()
                    * inv)
                    .sqrt()
            },
            max_genome_len: world
                .organisms
                .iter()
                .map(|o| o.genome.connections.len())
                .max()
                .unwrap_or(0),
            mean_hidden_used: world
                .organisms
                .iter()
                .map(|o| f64::from(hidden_alive(o)))
                .sum::<f64>()
                * inv,
            corr_brain_gain: corr_brain_gain(world),
            live_conn_frac,
        }
    }

    pub fn csv_header() -> &'static str {
        "tick,population,births,deaths_starve,deaths_age,deaths_predation,mean_energy,\
         max_energy,mean_genome_len,diversity,mean_size,mean_speed_gene,mean_actual_speed,\
         mean_metab,mean_max_age,mean_generation,food_count,detritus_count,\
         world_energy,injected,radiated,drift,bites,predation_flux,\
         mean_bite_hue_dist,mean_neighbor_hue_dist,std_size,std_speed,\
         std_genome_len,max_genome_len,mean_hidden_used,corr_brain_gain,live_conn_frac"
    }

    /// One CSV row. Event counts and flux are reported per window: the
    /// difference against `prev`, or the cumulative total for the first
    /// row. The hue-distance columns are per-window means; compare
    /// `mean_bite_hue_dist` against `mean_neighbor_hue_dist` — bites
    /// choosing *more* distant hues than the ambient neighborhood is the
    /// kin-discrimination signal.
    pub fn csv_row(&self, prev: Option<&Metrics>) -> String {
        let zero = Metrics::zeroed();
        let p = prev.unwrap_or(&zero);
        let b = self.births_total - p.births_total;
        let ds = self.deaths_starve_total - p.deaths_starve_total;
        let da = self.deaths_age_total - p.deaths_age_total;
        let dp = self.deaths_predation_total - p.deaths_predation_total;
        let bites = self.bites_total - p.bites_total;
        let flux = self.predation_flux_total - p.predation_flux_total;
        let bite_hue = if bites > 0 {
            (self.bite_hue_sum_total - p.bite_hue_sum_total) / bites as f64
        } else {
            0.0
        };
        let nbr = self.neighbor_hue_count_total - p.neighbor_hue_count_total;
        let nbr_hue = if nbr > 0 {
            (self.neighbor_hue_sum_total - p.neighbor_hue_sum_total) / nbr as f64
        } else {
            0.0
        };
        format!(
            "{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.6},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{},{},{:.4},{:.4},{:.4},{:.6e},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.3},{},{:.2},{:.4},{:.4}",
            self.tick,
            self.population,
            b,
            ds,
            da,
            dp,
            self.mean_energy,
            self.max_energy,
            self.mean_genome_len,
            self.diversity,
            self.mean_size,
            self.mean_speed_gene,
            self.mean_actual_speed,
            self.mean_metab,
            self.mean_max_age,
            self.mean_generation,
            self.food_count,
            self.detritus_count,
            self.world_energy,
            self.injected,
            self.radiated,
            self.drift,
            bites,
            flux,
            bite_hue,
            nbr_hue,
            self.std_size_log,
            self.std_speed_log,
            self.std_genome_len,
            self.max_genome_len,
            self.mean_hidden_used,
            self.corr_brain_gain,
            self.live_conn_frac,
        )
    }

    /// A prev-row stand-in for the first sample, so cumulative counters
    /// report as their own first window.
    fn zeroed() -> Metrics {
        Metrics {
            tick: 0,
            population: 0,
            births_total: 0,
            deaths_starve_total: 0,
            deaths_age_total: 0,
            deaths_predation_total: 0,
            mean_energy: 0.0,
            max_energy: 0.0,
            mean_genome_len: 0.0,
            diversity: 0.0,
            mean_size: 0.0,
            mean_speed_gene: 0.0,
            mean_actual_speed: 0.0,
            mean_metab: 0.0,
            mean_max_age: 0.0,
            mean_generation: 0.0,
            food_count: 0,
            detritus_count: 0,
            world_energy: 0.0,
            injected: 0.0,
            radiated: 0.0,
            drift: 0.0,
            bites_total: 0,
            predation_flux_total: 0.0,
            bite_hue_sum_total: 0.0,
            neighbor_hue_sum_total: 0.0,
            neighbor_hue_count_total: 0,
            std_size_log: 0.0,
            std_speed_log: 0.0,
            std_genome_len: 0.0,
            max_genome_len: 0,
            mean_hidden_used: 0.0,
            corr_brain_gain: 0.0,
            live_conn_frac: 0.0,
        }
    }
}

/// Hidden neurons of this organism that are actually alive: fed by at
/// least one connection AND read by at least one.
fn hidden_alive(o: &crate::organism::Organism) -> u32 {
    let mut in_deg = [0u32; N_HIDDEN];
    let mut out_deg = [0u32; N_HIDDEN];
    for g in &o.genome.connections {
        if !g.to_output {
            in_deg[g.to as usize] += 1;
        }
        if g.from_hidden {
            out_deg[g.from as usize] += 1;
        }
    }
    (0..N_HIDDEN)
        .filter(|&k| in_deg[k] >= 1 && out_deg[k] >= 1)
        .count() as u32
}

/// Pearson correlation between connection count and bite income per tick
/// of life, over the whole population. RNG-free; 0 when undefined.
fn corr_brain_gain(world: &World) -> f64 {
    let n = world.organisms.len();
    if n < 3 {
        return 0.0;
    }
    let inv = 1.0 / n as f64;
    let xs: Vec<f64> = world
        .organisms
        .iter()
        .map(|o| o.genome.connections.len() as f64)
        .collect();
    let ys: Vec<f64> = world
        .organisms
        .iter()
        .map(|o| o.lifetime_bite_gain / f64::from(o.age.max(1)))
        .collect();
    let mx = xs.iter().sum::<f64>() * inv;
    let my = ys.iter().sum::<f64>() * inv;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for (x, y) in xs.iter().zip(&ys) {
        cov += (x - mx) * (y - my);
        vx += (x - mx) * (x - mx);
        vy += (y - my) * (y - my);
    }
    if vx <= 0.0 || vy <= 0.0 {
        return 0.0;
    }
    cov / (vx.sqrt() * vy.sqrt())
}

fn diversity_and_live(world: &World) -> (f64, f64) {
    let n = world.organisms.len();
    if n < 2 {
        return (0.0, 0.0);
    }
    // A throwaway stream keyed to (seed, tick): deterministic, and
    // independent of how often anyone looks.
    let mut rng = Rng::from_seed(
        world
            .seed
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add(world.tick),
    );
    let k = n.min(DIVERSITY_SAMPLE);
    let mut idx: Vec<u32> = (0..n as u32).collect();
    for t in 0..k {
        let j = t + rng.below(n - t);
        idx.swap(t, j);
    }
    let sample = &idx[..k];
    let sorted: Vec<_> = sample
        .iter()
        .map(|&i| sorted_connections(&world.organisms[i as usize].genome))
        .collect();
    let mut sum = 0.0f64;
    let mut count = 0u64;
    for a in 0..k {
        for b in (a + 1)..k {
            let ga = &world.organisms[sample[a] as usize].genome;
            let gb = &world.organisms[sample[b] as usize].genome;
            sum += genome_distance(ga, &sorted[a], gb, &sorted[b], &world.cfg) as f64;
            count += 1;
        }
    }

    // Live-wiring fraction over the same sample: a gene is live if its
    // weight is non-negligible and it doesn't feed or read a hidden neuron
    // nothing completes. Same classification as `vivere inspect`.
    let mut live = 0u64;
    let mut total = 0u64;
    for &i in sample {
        let o = &world.organisms[i as usize];
        let mut in_deg = [0u32; N_HIDDEN];
        let mut out_deg = [0u32; N_HIDDEN];
        for g in &o.genome.connections {
            if !g.to_output {
                in_deg[g.to as usize] += 1;
            }
            if g.from_hidden {
                out_deg[g.from as usize] += 1;
            }
        }
        for g in &o.genome.connections {
            total += 1;
            let weak = g.weight.abs() < 0.01;
            let dead = (g.from_hidden && in_deg[g.from as usize] == 0)
                || (!g.to_output && out_deg[g.to as usize] == 0);
            if !weak && !dead {
                live += 1;
            }
        }
    }
    let live_frac = if total > 0 {
        live as f64 / total as f64
    } else {
        0.0
    };
    (sum / count as f64, live_frac)
}
