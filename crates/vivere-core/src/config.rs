//! All simulation constants, split by what they mean to the project doctrine:
//!
//! - [`EnvCfg`] is the *environment* — the only knobs turned when a world
//!   fails to sustain life. A struggling world gets a different climate,
//!   never different organisms.
//! - Everything else ([`BodyCfg`], [`ReproCfg`], [`MutationCfg`]) is frozen
//!   physics and heredity. These numbers define what bodies cost and how
//!   inheritance errs; they are set once, from budget arithmetic, and are
//!   not tuned to produce outcomes.
//!
//! The whole config is embedded in snapshots, so a saved world carries its
//! own physics.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub world: WorldCfg,
    pub env: EnvCfg,
    pub body: BodyCfg,
    pub repro: ReproCfg,
    pub mutation: MutationCfg,
}

/// World geometry and initial conditions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldCfg {
    pub width: f32,
    pub height: f32,
    /// How far organisms can sense. Also the spatial-grid cell size.
    pub sense_radius: f32,
    pub initial_organisms: u32,
    pub initial_food: u32,
    /// Starting energy of founders, as a fraction of their capacity.
    pub initial_energy_frac: f64,
}

/// The tunable environment: energy inflow, food growth, decay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvCfg {
    /// Energy entering the world per tick (as grown food), when below the cap.
    pub influx_per_tick: f64,
    /// Energy of a freshly grown food pellet.
    pub food_energy: f64,
    /// Sunlight only grows food while fewer than this many pellets exist.
    /// Compost is exempt: the cap gates *influx*, not recycling.
    pub food_cap: u32,
    /// Ambient light level; lower base = starker contrast with patches.
    pub light_base: f32,
    pub light_patches: u32,
    pub light_patch_amp: f32,
    pub light_patch_sigma: f32,
    /// Ticks a corpse takes to decay back into food.
    pub compost_delay: u32,
    /// Fraction of detritus energy that returns as food; the rest is heat.
    pub compost_efficiency: f64,
}

/// What bodies cost. These constants create the tradeoffs: bigger stores
/// more but burns more, faster movement is quadratically dearer, longevity
/// and brains have upkeep. The size-independent floor keeps "be tiny" from
/// being a free lunch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodyCfg {
    pub size_min: f32,
    pub size_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub metab_min: f32,
    pub metab_max: f32,
    pub age_min: f32,
    pub age_max: f32,
    /// Energy storage capacity per unit size.
    pub capacity_per_size: f64,
    /// Structural energy locked into a body per unit size; paid by the
    /// parent at birth, returned to the world as detritus at death.
    pub soma_per_size: f64,
    /// Physical radius per unit size (world units).
    pub radius_per_size: f32,
    /// Radius of a food pellet, for contact checks.
    pub food_radius: f32,
    /// Base upkeep per tick, regardless of size.
    pub upkeep_floor: f64,
    /// Upkeep per tick per (size × metabolic rate).
    pub upkeep_metab: f64,
    /// Upkeep per tick per (size × max_age/1000): longevity has maintenance
    /// costs, priced in absolute ticks so the gene-range walls are never
    /// load-bearing physics.
    pub upkeep_age: f64,
    /// Upkeep per tick per (size × max_speed): fast-twitch capacity must be
    /// fed even when unused, so idle muscle decays out of populations.
    pub upkeep_speed_capacity: f64,
    /// Upkeep per tick per brain connection: computation costs energy.
    pub upkeep_per_connection: f64,
    /// Movement cost coefficient: cost = c · size · v² per tick.
    pub move_cost: f64,
    /// Eating rate per (metab × size) per tick while touching food.
    pub bite_rate: f64,
    /// Maximum turn per tick (radians).
    pub turn_rate: f32,
    /// Period of the oscillator sense (ticks).
    pub oscillator_period: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReproCfg {
    /// Reproduce when energy exceeds this fraction of capacity.
    pub threshold_frac: f64,
    /// Child's starting energy as a fraction of the *child's* capacity.
    pub child_energy_frac: f64,
    /// Fixed birth overhead lost as heat.
    pub overhead: f64,
    /// Child spawns this far behind the parent.
    pub spawn_offset: f32,
}

/// Heredity physics: how copying errs. Frozen along with body constants —
/// mutation rates are not an environment knob.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationCfg {
    /// Per-connection chance of a weight perturbation.
    pub weight_p: f32,
    pub weight_sigma: f32,
    /// Per-connection chance of re-targeting one endpoint.
    pub rewire_p: f32,
    /// Per-child chance of gaining a random connection.
    pub add_p: f32,
    /// Per-child chance of losing a random connection.
    pub remove_p: f32,
    /// Per-child chance of duplicating a connection (gene duplication).
    pub duplicate_p: f32,
    /// Per-body-gene chance of a perturbation.
    pub body_p: f32,
    /// Log-space sigma for the four scale genes (size, speed, metab,
    /// max_age): mutation is multiplicative, reflected at the range walls.
    pub scale_sigma_log: f32,
    /// Additive sigma for hue (circular, wraps).
    pub hue_sigma: f32,
    /// Hard cap on connection count (bounds compute; far above typical use).
    pub genome_cap: usize,
    /// Connections in a founder's random genome.
    pub initial_connections: usize,
    /// Weights clamp to ±this.
    pub weight_max: f32,
}

// Carrying-capacity arithmetic behind the defaults (v0.2):
// a log-uniform founder averages (geometric means) size 1.41, speed 1.34,
// metab 0.87, max_age ≈ 2700. Its basal burn is
//   0.010                              floor
// + 1.41·(0.012·0.87 + 0.0007·2.7)   ≈ 0.017   metabolism + longevity
// + 0.002·1.41·1.34                  ≈ 0.004   idle speed capacity
// + 0.0004·12                        ≈ 0.005   brain
// ≈ 0.036/tick, plus ~0.004 movement at half throttle → ~0.040/tick.
// Influx 32/tick then supports K ≈ 800. The v0.1 mature grazer (size 2.38
// hauling an unused 2.94 speed gene) pays ~+30% under the speed-capacity
// term — deliberately: capacity you never use should not be free.
impl Default for Config {
    fn default() -> Self {
        Self {
            world: WorldCfg {
                width: 1200.0,
                height: 800.0,
                sense_radius: 64.0,
                initial_organisms: 250,
                initial_food: 900,
                initial_energy_frac: 0.6,
            },
            env: EnvCfg {
                influx_per_tick: 32.0,
                food_energy: 12.0,
                food_cap: 2600,
                light_base: 0.15,
                light_patches: 3,
                light_patch_amp: 0.85,
                light_patch_sigma: 170.0,
                compost_delay: 200,
                compost_efficiency: 0.85,
            },
            body: BodyCfg {
                size_min: 0.5,
                size_max: 4.0,
                speed_min: 0.3,
                speed_max: 6.0,
                metab_min: 0.25,
                metab_max: 3.0,
                age_min: 500.0,
                age_max: 15000.0,
                capacity_per_size: 40.0,
                soma_per_size: 8.0,
                radius_per_size: 3.0,
                food_radius: 2.0,
                upkeep_floor: 0.010,
                upkeep_metab: 0.012,
                upkeep_age: 0.0007,
                upkeep_speed_capacity: 0.002,
                upkeep_per_connection: 0.0004,
                move_cost: 0.004,
                bite_rate: 3.0,
                turn_rate: 0.25,
                oscillator_period: 60.0,
            },
            repro: ReproCfg {
                threshold_frac: 0.75,
                child_energy_frac: 0.35,
                overhead: 1.0,
                spawn_offset: 6.0,
            },
            mutation: MutationCfg {
                weight_p: 0.05,
                weight_sigma: 0.3,
                rewire_p: 0.01,
                add_p: 0.08,
                remove_p: 0.05,
                duplicate_p: 0.02,
                body_p: 0.08,
                scale_sigma_log: 0.06,
                hue_sigma: 0.05,
                genome_cap: 64,
                initial_connections: 12,
                weight_max: 4.0,
            },
        }
    }
}

impl Config {
    /// A miniature world for fast tests: same physics, smaller stage.
    pub fn test_small() -> Self {
        let mut c = Self::default();
        c.world.width = 400.0;
        c.world.height = 300.0;
        c.world.initial_organisms = 60;
        c.world.initial_food = 220;
        c.env.influx_per_tick = 8.0;
        c.env.food_cap = 650;
        c
    }
}
