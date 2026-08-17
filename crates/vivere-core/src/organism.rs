//! An organism is position, heading, energy, age, and a genome — nothing
//! else. Its costs are derived from its genes through `BodyCfg`; there is
//! no fitness, no score, no goal. The `last_*` fields are observation-only
//! caches for the viewer's inspector and are excluded from snapshots.

use crate::brain::{BrainState, N_SENSES};
use crate::config::Config;
use crate::genome::Genome;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organism {
    pub id: u64,
    pub generation: u32,
    pub x: f32,
    pub y: f32,
    pub heading: f32,
    pub energy: f64,
    /// Structural energy locked in the body; returns as detritus at death.
    pub soma: f64,
    pub age: u32,
    pub genome: Genome,
    pub brain: BrainState,
    #[serde(skip)]
    pub last_senses: [f32; N_SENSES],
    #[serde(skip)]
    pub last_turn: f32,
    #[serde(skip)]
    pub last_speed: f32,
}

impl Organism {
    pub fn capacity(&self, cfg: &Config) -> f64 {
        cfg.body.capacity_per_size * self.genome.body.size as f64
    }

    pub fn radius(&self, cfg: &Config) -> f32 {
        cfg.body.radius_per_size * self.genome.body.size
    }

    /// Basal metabolic burn per tick. The floor is size-independent so
    /// miniaturization can't escape the cost of being alive; metabolic
    /// rate, longevity, and brain size each add their share.
    pub fn upkeep(&self, cfg: &Config) -> f64 {
        let b = &cfg.body;
        let g = &self.genome.body;
        let age_norm = ((g.max_age - b.age_min) / (b.age_max - b.age_min)) as f64;
        b.upkeep_floor
            + g.size as f64 * (b.upkeep_metab * g.metab as f64 + b.upkeep_age * age_norm)
            + b.upkeep_per_connection * self.genome.connections.len() as f64
    }

    /// Energy this organism can extract from food per tick while touching it.
    pub fn bite_budget(&self, cfg: &Config) -> f64 {
        cfg.body.bite_rate * (self.genome.body.metab * self.genome.body.size) as f64
    }
}

pub fn soma_for(size: f32, cfg: &Config) -> f64 {
    cfg.body.soma_per_size * size as f64
}
