//! v0.1 → v0.2 snapshot importer.
//!
//! postcard is positional: decoding depends on exact struct shapes, so this
//! module keeps **frozen verbatim copies** of every v0.1 serialized type
//! (`V1*`, deserialize-only) — never the live core types, which are free to
//! evolve. The write side mirrors the *current* serialized `World` layout
//! (`V2World`) and round-trips the result through the official
//! `World::from_snapshot_bytes`, so the core deserializer validates the
//! conversion.
//!
//! Two guards make silent misalignment loud:
//! - every gene must lie within the v0.1 walls recorded in the old config;
//! - the imported world's energy books must balance (conservation is a free
//!   checksum: a shifted decode almost certainly breaks it).
//!
//! Imported worlds get **v0.2 physics with their v0.1 environment**: same
//! climate, new costs. That is the experiment.

use serde::{Deserialize, Serialize};
use vivere_core::brain::BrainState;
use vivere_core::config::Config;
use vivere_core::genome::{Body, Connection, Genome};
use vivere_core::world::{Detritus, Food, LightField, LightPatch, SNAPSHOT_MAGIC, World};

const V1_MAGIC: &[u8; 8] = b"VIVERE01";

// ---- frozen v0.1 shapes (deserialize only) --------------------------------

#[derive(Deserialize, Serialize)]
struct RawRng {
    s: [u64; 4],
}

#[derive(Deserialize)]
struct V1World {
    cfg: V1Config,
    seed: u64,
    tick: u64,
    rng: RawRng,
    organisms: Vec<V1Organism>,
    food: Vec<V1Food>,
    detritus: Vec<V1Detritus>,
    light: V1LightField,
    ledger: V1Ledger,
    next_id: u64,
    influx_accum: f64,
    births: u64,
    deaths_starve: u64,
    deaths_age: u64,
}

#[derive(Deserialize)]
struct V1Config {
    world: V1WorldCfg,
    env: V1EnvCfg,
    body: V1BodyCfg,
    repro: V1ReproCfg,
    #[allow(dead_code)]
    mutation: V1MutationCfg,
}

#[derive(Deserialize)]
struct V1WorldCfg {
    width: f32,
    height: f32,
    sense_radius: f32,
    initial_organisms: u32,
    initial_food: u32,
    initial_energy_frac: f64,
}

#[derive(Deserialize)]
struct V1EnvCfg {
    influx_per_tick: f64,
    food_energy: f64,
    food_cap: u32,
    light_base: f32,
    light_patches: u32,
    light_patch_amp: f32,
    light_patch_sigma: f32,
    compost_delay: u32,
    compost_efficiency: f64,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct V1BodyCfg {
    size_min: f32,
    size_max: f32,
    speed_min: f32,
    speed_max: f32,
    metab_min: f32,
    metab_max: f32,
    age_min: f32,
    age_max: f32,
    capacity_per_size: f64,
    soma_per_size: f64,
    radius_per_size: f32,
    food_radius: f32,
    upkeep_floor: f64,
    upkeep_metab: f64,
    upkeep_age: f64,
    upkeep_per_connection: f64,
    move_cost: f64,
    bite_rate: f64,
    turn_rate: f32,
    oscillator_period: f32,
}

#[derive(Deserialize)]
struct V1ReproCfg {
    threshold_frac: f64,
    child_energy_frac: f64,
    overhead: f64,
    spawn_offset: f32,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct V1MutationCfg {
    weight_p: f32,
    weight_sigma: f32,
    rewire_p: f32,
    add_p: f32,
    remove_p: f32,
    duplicate_p: f32,
    body_p: f32,
    body_sigma_frac: f32,
    genome_cap: usize,
    initial_connections: usize,
    weight_max: f32,
}

#[derive(Deserialize)]
struct V1Organism {
    id: u64,
    generation: u32,
    x: f32,
    y: f32,
    heading: f32,
    energy: f64,
    soma: f64,
    age: u32,
    genome: V1Genome,
    brain: V1Brain,
}

#[derive(Deserialize)]
struct V1Brain {
    hidden: [f32; 8],
}

#[derive(Deserialize)]
struct V1Genome {
    connections: Vec<V1Connection>,
    body: V1Body,
}

#[derive(Deserialize)]
struct V1Connection {
    from_hidden: bool,
    from: u8,
    to_output: bool,
    to: u8,
    weight: f32,
}

#[derive(Deserialize)]
struct V1Body {
    size: f32,
    max_speed: f32,
    metab: f32,
    max_age: f32,
    hue: f32,
}

#[derive(Deserialize)]
struct V1Food {
    x: f32,
    y: f32,
    energy: f64,
}

#[derive(Deserialize)]
struct V1Detritus {
    x: f32,
    y: f32,
    energy: f64,
    ticks_left: u32,
}

#[derive(Deserialize)]
struct V1LightField {
    base: f32,
    patches: Vec<V1LightPatch>,
}

#[derive(Deserialize)]
struct V1LightPatch {
    x: f32,
    y: f32,
    amp: f32,
    sigma: f32,
}

#[derive(Deserialize)]
struct V1Ledger {
    injected: f64,
    radiated: f64,
}

// ---- current serialized World layout (serialize only) ---------------------
// Field order must mirror `World`'s serialized (non-skip) fields exactly;
// the round-trip through `World::from_snapshot_bytes` enforces agreement.

#[derive(Serialize)]
struct V2World {
    cfg: Config,
    seed: u64,
    tick: u64,
    rng: RawRng,
    organisms: Vec<V2Organism>,
    food: Vec<Food>,
    detritus: Vec<Detritus>,
    light: LightField,
    ledger: V2Ledger,
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
struct V2Ledger {
    injected: f64,
    radiated: f64,
}

#[derive(Serialize)]
struct V2Organism {
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
    drained_last_tick: f64,
}

/// Convert v0.1 snapshot bytes into a live v0.2 `World`.
pub fn import_v01(bytes: &[u8], contact_enabled: bool) -> Result<World, String> {
    if bytes.len() < V1_MAGIC.len() || &bytes[..V1_MAGIC.len()] != V1_MAGIC {
        return Err("not a v0.1 vivere snapshot (magic VIVERE01 missing)".into());
    }
    let v1: V1World = postcard::from_bytes(&bytes[V1_MAGIC.len()..])
        .map_err(|e| format!("v0.1 snapshot failed to decode: {e}"))?;

    // Guard 1: every gene inside the walls the old world recorded.
    let b = &v1.cfg.body;
    for o in &v1.organisms {
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
                "organism {} has genes outside the v0.1 walls — misaligned decode?",
                o.id
            ));
        }
    }

    // v0.2 physics, v0.1 environment. That asymmetry is the experiment.
    let mut cfg = Config::default();
    cfg.world.width = v1.cfg.world.width;
    cfg.world.height = v1.cfg.world.height;
    cfg.world.sense_radius = v1.cfg.world.sense_radius;
    cfg.world.initial_organisms = v1.cfg.world.initial_organisms;
    cfg.world.initial_food = v1.cfg.world.initial_food;
    cfg.world.initial_energy_frac = v1.cfg.world.initial_energy_frac;
    cfg.env.influx_per_tick = v1.cfg.env.influx_per_tick;
    cfg.env.food_energy = v1.cfg.env.food_energy;
    cfg.env.food_cap = v1.cfg.env.food_cap;
    cfg.env.light_base = v1.cfg.env.light_base;
    cfg.env.light_patches = v1.cfg.env.light_patches;
    cfg.env.light_patch_amp = v1.cfg.env.light_patch_amp;
    cfg.env.light_patch_sigma = v1.cfg.env.light_patch_sigma;
    cfg.env.compost_delay = v1.cfg.env.compost_delay;
    cfg.env.compost_efficiency = v1.cfg.env.compost_efficiency;
    cfg.repro.threshold_frac = v1.cfg.repro.threshold_frac;
    cfg.repro.child_energy_frac = v1.cfg.repro.child_energy_frac;
    cfg.repro.overhead = v1.cfg.repro.overhead;
    cfg.repro.spawn_offset = v1.cfg.repro.spawn_offset;
    cfg.contact.enabled = contact_enabled;

    let organisms = v1
        .organisms
        .into_iter()
        .map(|o| V2Organism {
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
            brain: BrainState {
                hidden: o.brain.hidden,
            },
            last_bitten_tick: u64::MAX,
            lifetime_drained: 0.0,
            drained_last_tick: 0.0,
        })
        .collect();

    let v2 = V2World {
        cfg,
        seed: v1.seed,
        tick: v1.tick,
        rng: v1.rng,
        organisms,
        food: v1
            .food
            .into_iter()
            .map(|f| Food {
                x: f.x,
                y: f.y,
                energy: f.energy,
            })
            .collect(),
        detritus: v1
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
            base: v1.light.base,
            patches: v1
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
        ledger: V2Ledger {
            injected: v1.ledger.injected,
            radiated: v1.ledger.radiated,
        },
        next_id: v1.next_id,
        influx_accum: v1.influx_accum,
        births: v1.births,
        deaths_starve: v1.deaths_starve,
        deaths_age: v1.deaths_age,
        deaths_predation: 0,
        bites: 0,
        predation_flux: 0.0,
        bite_hue_sum: 0.0,
        neighbor_hue_sum: 0.0,
        neighbor_hue_count: 0,
    };

    let mut out = SNAPSHOT_MAGIC.to_vec();
    out.extend(postcard::to_allocvec(&v2).map_err(|e| format!("re-encode failed: {e}"))?);
    let world = World::from_snapshot_bytes(&out)?;

    // Guard 2: the books must balance in the imported world.
    let err = world.conservation_error().abs();
    let bound = 1e-6 * world.ledger.injected.max(1.0);
    if err > bound {
        return Err(format!(
            "imported world leaks energy ({err:.3e} > {bound:.3e}) — misaligned decode?"
        ));
    }
    Ok(world)
}
