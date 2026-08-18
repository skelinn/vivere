//! v0.1 snapshot importer: frozen v0.1 shapes, converted **forward into the
//! frozen v0.2 shapes** owned by [`crate::legacy_v02`], which alone writes
//! live core types. Every future format bump leaves this module untouched —
//! v0.1 imports compose through the chain.
//!
//! The intermediate `V02Config` built here is a carrier: `from_v02` only
//! reads its world/env/repro/contact sections and its body *walls* (the
//! gene guard); every other constant is replaced by current physics.

use crate::legacy_v02::{self, RawRng};
use serde::Deserialize;
use vivere_core::world::World;

const V1_MAGIC: &[u8; 8] = b"VIVERE01";

// ---- frozen v0.1 shapes (deserialize only) --------------------------------

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

/// Convert v0.1 snapshot bytes into a live current-format `World`, via the
/// frozen v0.2 shapes. v0.1 predates contact physics, so the caller decides
/// whether the imported world has the channel enabled.
pub fn import_v01(bytes: &[u8], contact_enabled: bool) -> Result<World, String> {
    if bytes.len() < V1_MAGIC.len() || &bytes[..V1_MAGIC.len()] != V1_MAGIC {
        return Err("not a v0.1 vivere snapshot (magic VIVERE01 missing)".into());
    }
    let v1: V1World = postcard::from_bytes(&bytes[V1_MAGIC.len()..])
        .map_err(|e| format!("v0.1 snapshot failed to decode: {e}"))?;

    // Guard: every gene inside the walls the old world recorded.
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

    let v02 = legacy_v02::V02World {
        cfg: legacy_v02::V02Config {
            world: legacy_v02::V02WorldCfg {
                width: v1.cfg.world.width,
                height: v1.cfg.world.height,
                sense_radius: v1.cfg.world.sense_radius,
                initial_organisms: v1.cfg.world.initial_organisms,
                initial_food: v1.cfg.world.initial_food,
                initial_energy_frac: v1.cfg.world.initial_energy_frac,
            },
            env: legacy_v02::V02EnvCfg {
                influx_per_tick: v1.cfg.env.influx_per_tick,
                food_energy: v1.cfg.env.food_energy,
                food_cap: v1.cfg.env.food_cap,
                light_base: v1.cfg.env.light_base,
                light_patches: v1.cfg.env.light_patches,
                light_patch_amp: v1.cfg.env.light_patch_amp,
                light_patch_sigma: v1.cfg.env.light_patch_sigma,
                compost_delay: v1.cfg.env.compost_delay,
                compost_efficiency: v1.cfg.env.compost_efficiency,
            },
            // Carrier values: only the walls (already validated above) are
            // read downstream; cost constants are replaced by current
            // physics in from_v02.
            body: legacy_v02::V02BodyCfg {
                size_min: v1.cfg.body.size_min,
                size_max: v1.cfg.body.size_max,
                speed_min: v1.cfg.body.speed_min,
                speed_max: v1.cfg.body.speed_max,
                metab_min: v1.cfg.body.metab_min,
                metab_max: v1.cfg.body.metab_max,
                age_min: v1.cfg.body.age_min,
                age_max: v1.cfg.body.age_max,
                capacity_per_size: v1.cfg.body.capacity_per_size,
                soma_per_size: v1.cfg.body.soma_per_size,
                radius_per_size: v1.cfg.body.radius_per_size,
                food_radius: v1.cfg.body.food_radius,
                upkeep_floor: v1.cfg.body.upkeep_floor,
                upkeep_metab: v1.cfg.body.upkeep_metab,
                upkeep_age: v1.cfg.body.upkeep_age,
                upkeep_speed_capacity: 0.002,
                upkeep_per_connection: v1.cfg.body.upkeep_per_connection,
                move_cost: v1.cfg.body.move_cost,
                bite_rate: v1.cfg.body.bite_rate,
                turn_rate: v1.cfg.body.turn_rate,
                oscillator_period: v1.cfg.body.oscillator_period,
            },
            repro: legacy_v02::V02ReproCfg {
                threshold_frac: v1.cfg.repro.threshold_frac,
                child_energy_frac: v1.cfg.repro.child_energy_frac,
                overhead: v1.cfg.repro.overhead,
                spawn_offset: v1.cfg.repro.spawn_offset,
            },
            mutation: legacy_v02::V02MutationCfg {
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
            contact: legacy_v02::V02ContactCfg {
                enabled: contact_enabled,
                bite_flux: 0.6,
                flesh_efficiency: 0.75,
                lunge_cost: 0.05,
            },
        },
        seed: v1.seed,
        tick: v1.tick,
        rng: v1.rng,
        organisms: v1
            .organisms
            .into_iter()
            .map(|o| legacy_v02::V02Organism {
                id: o.id,
                generation: o.generation,
                x: o.x,
                y: o.y,
                heading: o.heading,
                energy: o.energy,
                soma: o.soma,
                age: o.age,
                genome: legacy_v02::V02Genome {
                    connections: o
                        .genome
                        .connections
                        .into_iter()
                        .map(|c| legacy_v02::V02Connection {
                            from_hidden: c.from_hidden,
                            from: c.from,
                            to_output: c.to_output,
                            to: c.to,
                            weight: c.weight,
                        })
                        .collect(),
                    body: legacy_v02::V02Body {
                        size: o.genome.body.size,
                        max_speed: o.genome.body.max_speed,
                        metab: o.genome.body.metab,
                        max_age: o.genome.body.max_age,
                        hue: o.genome.body.hue,
                    },
                },
                brain: legacy_v02::V02Brain {
                    hidden: o.brain.hidden,
                },
                last_bitten_tick: u64::MAX,
                lifetime_drained: 0.0,
                drained_last_tick: 0.0,
            })
            .collect(),
        food: v1
            .food
            .into_iter()
            .map(|f| legacy_v02::V02Food {
                x: f.x,
                y: f.y,
                energy: f.energy,
            })
            .collect(),
        detritus: v1
            .detritus
            .into_iter()
            .map(|d| legacy_v02::V02Detritus {
                x: d.x,
                y: d.y,
                energy: d.energy,
                ticks_left: d.ticks_left,
            })
            .collect(),
        light: legacy_v02::V02LightField {
            base: v1.light.base,
            patches: v1
                .light
                .patches
                .into_iter()
                .map(|p| legacy_v02::V02LightPatch {
                    x: p.x,
                    y: p.y,
                    amp: p.amp,
                    sigma: p.sigma,
                })
                .collect(),
        },
        ledger: legacy_v02::V02Ledger {
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

    legacy_v02::from_v02(v02)
}
