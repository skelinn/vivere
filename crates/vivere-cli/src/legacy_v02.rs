//! v0.2 → current snapshot importer, and the **frozen v0.2 shapes**.
//!
//! Architecture (one module per retired format, O(1) maintenance): each
//! legacy module owns its format's frozen structs and converts *forward*
//! into the next format's frozen structs; only the newest module writes
//! live core types. `legacy_v01` builds `V02World` and hands it here, so
//! v0.1 imports compose through this module and every future format bump
//! touches exactly one writer.
//!
//! The v0.2 shapes derive Serialize *and* Deserialize: this module reads
//! them from disk, `legacy_v01` writes them in memory.
//!
//! Guards, as everywhere in the import chain: genes inside the walls the
//! old world recorded, and a balanced energy ledger after conversion — a
//! misaligned positional decode almost never survives both.

use serde::{Deserialize, Serialize};
use vivere_core::brain::{BrainState, N_HIDDEN};
use vivere_core::config::Config;
use vivere_core::genome::{Body, Connection, Genome};
use vivere_core::world::{Detritus, Food, LightField, LightPatch, SNAPSHOT_MAGIC, World};

pub(crate) const V02_MAGIC: &[u8; 8] = b"VIVERE02";
const V02_HIDDEN: usize = 8;

#[derive(Serialize, Deserialize)]
pub struct RawRng {
    pub s: [u64; 4],
}

// ---- frozen v0.2 shapes ---------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct V02World {
    pub cfg: V02Config,
    pub seed: u64,
    pub tick: u64,
    pub rng: RawRng,
    pub organisms: Vec<V02Organism>,
    pub food: Vec<V02Food>,
    pub detritus: Vec<V02Detritus>,
    pub light: V02LightField,
    pub ledger: V02Ledger,
    pub next_id: u64,
    pub influx_accum: f64,
    pub births: u64,
    pub deaths_starve: u64,
    pub deaths_age: u64,
    pub deaths_predation: u64,
    pub bites: u64,
    pub predation_flux: f64,
    pub bite_hue_sum: f64,
    pub neighbor_hue_sum: f64,
    pub neighbor_hue_count: u64,
}

#[derive(Serialize, Deserialize)]
pub struct V02Config {
    pub world: V02WorldCfg,
    pub env: V02EnvCfg,
    pub body: V02BodyCfg,
    pub repro: V02ReproCfg,
    pub mutation: V02MutationCfg,
    pub contact: V02ContactCfg,
}

#[derive(Serialize, Deserialize)]
pub struct V02WorldCfg {
    pub width: f32,
    pub height: f32,
    pub sense_radius: f32,
    pub initial_organisms: u32,
    pub initial_food: u32,
    pub initial_energy_frac: f64,
}

#[derive(Serialize, Deserialize)]
pub struct V02EnvCfg {
    pub influx_per_tick: f64,
    pub food_energy: f64,
    pub food_cap: u32,
    pub light_base: f32,
    pub light_patches: u32,
    pub light_patch_amp: f32,
    pub light_patch_sigma: f32,
    pub compost_delay: u32,
    pub compost_efficiency: f64,
}

#[derive(Serialize, Deserialize)]
pub struct V02BodyCfg {
    pub size_min: f32,
    pub size_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub metab_min: f32,
    pub metab_max: f32,
    pub age_min: f32,
    pub age_max: f32,
    pub capacity_per_size: f64,
    pub soma_per_size: f64,
    pub radius_per_size: f32,
    pub food_radius: f32,
    pub upkeep_floor: f64,
    pub upkeep_metab: f64,
    pub upkeep_age: f64,
    pub upkeep_speed_capacity: f64,
    pub upkeep_per_connection: f64,
    pub move_cost: f64,
    pub bite_rate: f64,
    pub turn_rate: f32,
    pub oscillator_period: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02ReproCfg {
    pub threshold_frac: f64,
    pub child_energy_frac: f64,
    pub overhead: f64,
    pub spawn_offset: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02MutationCfg {
    pub weight_p: f32,
    pub weight_sigma: f32,
    pub rewire_p: f32,
    pub add_p: f32,
    pub remove_p: f32,
    pub duplicate_p: f32,
    pub body_p: f32,
    pub scale_sigma_log: f32,
    pub hue_sigma: f32,
    pub genome_cap: usize,
    pub initial_connections: usize,
    pub weight_max: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02ContactCfg {
    pub enabled: bool,
    pub bite_flux: f64,
    pub flesh_efficiency: f64,
    pub lunge_cost: f64,
}

#[derive(Serialize, Deserialize)]
pub struct V02Organism {
    pub id: u64,
    pub generation: u32,
    pub x: f32,
    pub y: f32,
    pub heading: f32,
    pub energy: f64,
    pub soma: f64,
    pub age: u32,
    pub genome: V02Genome,
    pub brain: V02Brain,
    pub last_bitten_tick: u64,
    pub lifetime_drained: f64,
    pub drained_last_tick: f64,
}

#[derive(Serialize, Deserialize)]
pub struct V02Brain {
    pub hidden: [f32; V02_HIDDEN],
}

#[derive(Serialize, Deserialize)]
pub struct V02Genome {
    pub connections: Vec<V02Connection>,
    pub body: V02Body,
}

#[derive(Serialize, Deserialize)]
pub struct V02Connection {
    pub from_hidden: bool,
    pub from: u8,
    pub to_output: bool,
    pub to: u8,
    pub weight: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02Body {
    pub size: f32,
    pub max_speed: f32,
    pub metab: f32,
    pub max_age: f32,
    pub hue: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02Food {
    pub x: f32,
    pub y: f32,
    pub energy: f64,
}

#[derive(Serialize, Deserialize)]
pub struct V02Detritus {
    pub x: f32,
    pub y: f32,
    pub energy: f64,
    pub ticks_left: u32,
}

#[derive(Serialize, Deserialize)]
pub struct V02LightField {
    pub base: f32,
    pub patches: Vec<V02LightPatch>,
}

#[derive(Serialize, Deserialize)]
pub struct V02LightPatch {
    pub x: f32,
    pub y: f32,
    pub amp: f32,
    pub sigma: f32,
}

#[derive(Serialize, Deserialize)]
pub struct V02Ledger {
    pub injected: f64,
    pub radiated: f64,
}

// ---- current serialized layout (this module is the only live writer) ------
// Field order mirrors `World`'s / `Organism`'s serialized (non-skip) fields;
// the round-trip through `World::from_snapshot_bytes` enforces agreement.

#[derive(Serialize)]
struct V3World {
    cfg: Config,
    seed: u64,
    tick: u64,
    rng: RawRng,
    organisms: Vec<V3Organism>,
    food: Vec<Food>,
    detritus: Vec<Detritus>,
    light: LightField,
    ledger: V02Ledger,
    next_id: u64,
    influx_accum: f64,
    births: u64,
    deaths_starve: u64,
    deaths_age: u64,
    deaths_predation: u64,
    bites: u64,
    predation_flux: f64,
    bite_hue_sum: f64,
    neighbor_hue_sum: f64,
    neighbor_hue_count: u64,
}

#[derive(Serialize)]
struct V3Organism {
    id: u64,
    generation: u32,
    x: f32,
    y: f32,
    heading: f32,
    energy: f64,
    soma: f64,
    age: u32,
    genome: Genome,
    brain: BrainState,
    last_bitten_tick: u64,
    lifetime_drained: f64,
    lifetime_bite_gain: f64,
    drained_last_tick: f64,
}

pub fn import_v02(bytes: &[u8]) -> Result<World, String> {
    if bytes.len() < V02_MAGIC.len() || &bytes[..V02_MAGIC.len()] != V02_MAGIC {
        return Err("not a v0.2 vivere snapshot (magic VIVERE02 missing)".into());
    }
    let v2: V02World = postcard::from_bytes(&bytes[V02_MAGIC.len()..])
        .map_err(|e| format!("v0.2 snapshot failed to decode: {e}"))?;
    from_v02(v2)
}

/// Convert a decoded v0.2 world into a live current-format `World`.
/// Current physics, original environment — the repricing is the point.
pub fn from_v02(v2: V02World) -> Result<World, String> {
    let b = &v2.cfg.body;
    for o in &v2.organisms {
        let g = &o.genome.body;
        let ok = g.size >= b.size_min
            && g.size <= b.size_max
            && g.max_speed >= b.speed_min
            && g.max_speed <= b.speed_max
            && g.metab >= b.metab_min
            && g.metab <= b.metab_max
            && g.max_age >= b.age_min
            && g.max_age <= b.age_max
            && (0.0..1.0).contains(&g.hue);
        if !ok {
            return Err(format!(
                "organism {} has genes outside the recorded v0.2 walls — misaligned decode?",
                o.id
            ));
        }
    }

    let mut cfg = Config::default();
    cfg.world.width = v2.cfg.world.width;
    cfg.world.height = v2.cfg.world.height;
    cfg.world.sense_radius = v2.cfg.world.sense_radius;
    cfg.world.initial_organisms = v2.cfg.world.initial_organisms;
    cfg.world.initial_food = v2.cfg.world.initial_food;
    cfg.world.initial_energy_frac = v2.cfg.world.initial_energy_frac;
    cfg.env.influx_per_tick = v2.cfg.env.influx_per_tick;
    cfg.env.food_energy = v2.cfg.env.food_energy;
    cfg.env.food_cap = v2.cfg.env.food_cap;
    cfg.env.light_base = v2.cfg.env.light_base;
    cfg.env.light_patches = v2.cfg.env.light_patches;
    cfg.env.light_patch_amp = v2.cfg.env.light_patch_amp;
    cfg.env.light_patch_sigma = v2.cfg.env.light_patch_sigma;
    cfg.env.compost_delay = v2.cfg.env.compost_delay;
    cfg.env.compost_efficiency = v2.cfg.env.compost_efficiency;
    cfg.repro.threshold_frac = v2.cfg.repro.threshold_frac;
    cfg.repro.child_energy_frac = v2.cfg.repro.child_energy_frac;
    cfg.repro.overhead = v2.cfg.repro.overhead;
    cfg.repro.spawn_offset = v2.cfg.repro.spawn_offset;
    cfg.contact.enabled = v2.cfg.contact.enabled;
    cfg.contact.bite_flux = v2.cfg.contact.bite_flux;
    cfg.contact.flesh_efficiency = v2.cfg.contact.flesh_efficiency;
    cfg.contact.lunge_cost = v2.cfg.contact.lunge_cost;

    let organisms = v2
        .organisms
        .into_iter()
        .map(|o| {
            let mut hidden = [0.0f32; N_HIDDEN];
            hidden[..V02_HIDDEN].copy_from_slice(&o.brain.hidden);
            V3Organism {
                id: o.id,
                generation: o.generation,
                x: o.x,
                y: o.y,
                heading: o.heading,
                energy: o.energy,
                soma: o.soma,
                age: o.age,
                genome: Genome {
                    connections: o
                        .genome
                        .connections
                        .into_iter()
                        .map(|c| Connection {
                            from_hidden: c.from_hidden,
                            from: c.from,
                            to_output: c.to_output,
                            to: c.to,
                            weight: c.weight,
                        })
                        .collect(),
                    body: Body {
                        size: o.genome.body.size,
                        max_speed: o.genome.body.max_speed,
                        metab: o.genome.body.metab,
                        max_age: o.genome.body.max_age,
                        hue: o.genome.body.hue,
                    },
                },
                brain: BrainState { hidden },
                last_bitten_tick: o.last_bitten_tick,
                lifetime_drained: o.lifetime_drained,
                lifetime_bite_gain: 0.0,
                drained_last_tick: o.drained_last_tick,
            }
        })
        .collect();

    let v3 = V3World {
        cfg,
        seed: v2.seed,
        tick: v2.tick,
        rng: v2.rng,
        organisms,
        food: v2
            .food
            .into_iter()
            .map(|f| Food {
                x: f.x,
                y: f.y,
                energy: f.energy,
            })
            .collect(),
        detritus: v2
            .detritus
            .into_iter()
            .map(|d| Detritus {
                x: d.x,
                y: d.y,
                energy: d.energy,
                ticks_left: d.ticks_left,
            })
            .collect(),
        light: LightField {
            base: v2.light.base,
            patches: v2
                .light
                .patches
                .into_iter()
                .map(|p| LightPatch {
                    x: p.x,
                    y: p.y,
                    amp: p.amp,
                    sigma: p.sigma,
                })
                .collect(),
        },
        ledger: v2.ledger,
        next_id: v2.next_id,
        influx_accum: v2.influx_accum,
        births: v2.births,
        deaths_starve: v2.deaths_starve,
        deaths_age: v2.deaths_age,
        deaths_predation: v2.deaths_predation,
        bites: v2.bites,
        predation_flux: v2.predation_flux,
        bite_hue_sum: v2.bite_hue_sum,
        neighbor_hue_sum: v2.neighbor_hue_sum,
        neighbor_hue_count: v2.neighbor_hue_count,
    };

    let mut out = SNAPSHOT_MAGIC.to_vec();
    out.extend(postcard::to_allocvec(&v3).map_err(|e| format!("re-encode failed: {e}"))?);
    let world = World::from_snapshot_bytes(&out)?;

    let err = world.conservation_error().abs();
    let bound = 1e-6 * world.ledger.injected.max(1.0);
    if err > bound {
        return Err(format!(
            "imported world leaks energy ({err:.3e} > {bound:.3e}) — misaligned decode?"
        ));
    }
    Ok(world)
}
