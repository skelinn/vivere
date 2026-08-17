//! Observation. Metrics read the world without touching it: the diversity
//! sample draws from a throwaway RNG derived from (seed, tick), so whether
//! and how often you measure can never change what happens.

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
}

impl Metrics {
    pub fn measure(world: &World) -> Metrics {
        let n = world.organisms.len();
        let inv = if n > 0 { 1.0 / n as f64 } else { 0.0 };
        let mean = |f: &dyn Fn(&crate::organism::Organism) -> f64| -> f64 {
            world.organisms.iter().map(f).sum::<f64>() * inv
        };

        Metrics {
            tick: world.tick,
            population: n,
            births_total: world.births,
            deaths_starve_total: world.deaths_starve,
            deaths_age_total: world.deaths_age,
            mean_energy: mean(&|o| o.energy),
            max_energy: world.organisms.iter().map(|o| o.energy).fold(0.0, f64::max),
            mean_genome_len: mean(&|o| o.genome.connections.len() as f64),
            diversity: diversity(world),
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
        }
    }

    pub fn csv_header() -> &'static str {
        "tick,population,births,deaths_starve,deaths_age,mean_energy,max_energy,\
         mean_genome_len,diversity,mean_size,mean_speed_gene,mean_actual_speed,\
         mean_metab,mean_max_age,mean_generation,food_count,detritus_count,\
         world_energy,injected,radiated,drift"
    }

    /// One CSV row. Event counts (births, deaths) are reported per window:
    /// the difference against `prev`, or the cumulative total for the first
    /// row.
    pub fn csv_row(&self, prev: Option<&Metrics>) -> String {
        let d = |cur: u64, pre: u64| cur - pre;
        let (b, ds, da) = match prev {
            Some(p) => (
                d(self.births_total, p.births_total),
                d(self.deaths_starve_total, p.deaths_starve_total),
                d(self.deaths_age_total, p.deaths_age_total),
            ),
            None => (
                self.births_total,
                self.deaths_starve_total,
                self.deaths_age_total,
            ),
        };
        format!(
            "{},{},{},{},{},{:.4},{:.4},{:.4},{:.6},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{},{},{:.4},{:.4},{:.4},{:.6e}",
            self.tick,
            self.population,
            b,
            ds,
            da,
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
        )
    }
}

fn diversity(world: &World) -> f64 {
    let n = world.organisms.len();
    if n < 2 {
        return 0.0;
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
    sum / count as f64
}
