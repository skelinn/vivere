//! The world and its tick pipeline. This file is where all energy flows
//! live; every transfer is an exact pair (what leaves one pool enters
//! another, or the ledger's `radiated`), and the debug build asserts
//! conservation after every tick.
//!
//! Tick phases, in fixed order:
//! 1. sunlight grows food (the only energy inflow)
//! 2. spatial grids rebuild
//! 3. every organism senses and thinks (against the same world state)
//! 4. every organism moves (and pays for it)
//! 5. eating, in a per-tick shuffled order (no standing index advantage)
//! 6. reproduction (mutated child, paid for exactly)
//! 7. upkeep and aging
//! 8. deaths → detritus
//! 9. detritus composts back into food
//!
//! Nothing in here scores, guides, or protects organisms.

use crate::brain::{self, BrainState, N_SENSES};
use crate::config::Config;
use crate::energy::Ledger;
use crate::genome::Genome;
use crate::grid::{Grid, wrap_angle, wrap_delta};
use crate::metrics::Metrics;
use crate::organism::{Organism, soma_for};
use crate::rng::Rng;
use core::f32::consts::{PI, TAU};
use serde::{Deserialize, Serialize};

pub const SNAPSHOT_MAGIC: &[u8; 8] = b"VIVERE01";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Food {
    pub x: f32,
    pub y: f32,
    pub energy: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Detritus {
    pub x: f32,
    pub y: f32,
    pub energy: f64,
    pub ticks_left: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LightPatch {
    pub x: f32,
    pub y: f32,
    pub amp: f32,
    pub sigma: f32,
}

/// Static, uneven sunlight: an ambient base plus a few bright patches,
/// placed once from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightField {
    pub base: f32,
    pub patches: Vec<LightPatch>,
}

impl LightField {
    pub fn sample(&self, x: f32, y: f32, w: f32, h: f32) -> f32 {
        let mut v = self.base;
        for p in &self.patches {
            let dx = wrap_delta(p.x, x, w);
            let dy = wrap_delta(p.y, y, h);
            v += p.amp * libm::expf(-(dx * dx + dy * dy) / (2.0 * p.sigma * p.sigma));
        }
        v
    }

    /// An upper bound on `sample`, for rejection sampling.
    pub fn max_value(&self) -> f32 {
        self.base + self.patches.iter().map(|p| p.amp).sum::<f32>()
    }
}

#[derive(Serialize, Deserialize)]
pub struct World {
    pub cfg: Config,
    pub seed: u64,
    pub tick: u64,
    pub rng: Rng,
    pub organisms: Vec<Organism>,
    pub food: Vec<Food>,
    pub detritus: Vec<Detritus>,
    pub light: LightField,
    pub ledger: Ledger,
    pub next_id: u64,
    /// Sunlight budget carried between ticks (fraction of a pellet).
    pub influx_accum: f64,
    pub births: u64,
    pub deaths_starve: u64,
    pub deaths_age: u64,

    // Transient scratch state: rebuilt or cleared every tick, never
    // serialized, and never observable by organisms.
    #[serde(skip)]
    org_grid: Grid,
    #[serde(skip)]
    food_grid: Grid,
    #[serde(skip)]
    decisions: Vec<(f32, f32)>,
    #[serde(skip)]
    order: Vec<u32>,
}

fn sample_light_pos(rng: &mut Rng, light: &LightField, w: f32, h: f32) -> (f32, f32) {
    let max = light.max_value();
    loop {
        let x = rng.range_f32(0.0, w);
        let y = rng.range_f32(0.0, h);
        if rng.f32() * max < light.sample(x, y, w, h) {
            return (x, y);
        }
    }
}

impl World {
    pub fn new(cfg: Config, seed: u64) -> Self {
        assert!(
            cfg.world.width >= 3.0 * cfg.world.sense_radius
                && cfg.world.height >= 3.0 * cfg.world.sense_radius,
            "world must be at least 3 sense radii across"
        );
        let (w, h) = (cfg.world.width, cfg.world.height);
        let mut rng = Rng::from_seed(seed);
        let mut ledger = Ledger::default();

        let light = LightField {
            base: cfg.env.light_base,
            patches: (0..cfg.env.light_patches)
                .map(|_| LightPatch {
                    x: rng.range_f32(0.0, w),
                    y: rng.range_f32(0.0, h),
                    amp: cfg.env.light_patch_amp,
                    sigma: cfg.env.light_patch_sigma,
                })
                .collect(),
        };

        let mut food = Vec::with_capacity(cfg.world.initial_food as usize);
        for _ in 0..cfg.world.initial_food {
            let (x, y) = sample_light_pos(&mut rng, &light, w, h);
            food.push(Food {
                x,
                y,
                energy: cfg.env.food_energy,
            });
            ledger.injected += cfg.env.food_energy;
        }

        let mut organisms = Vec::with_capacity(cfg.world.initial_organisms as usize);
        for i in 0..cfg.world.initial_organisms {
            let genome = Genome::random(&mut rng, &cfg);
            let soma = soma_for(genome.body.size, &cfg);
            let energy = cfg.world.initial_energy_frac
                * cfg.body.capacity_per_size
                * genome.body.size as f64;
            ledger.injected += energy + soma;
            organisms.push(Organism {
                id: i as u64,
                generation: 0,
                x: rng.range_f32(0.0, w),
                y: rng.range_f32(0.0, h),
                heading: rng.range_f32(-PI, PI),
                energy,
                soma,
                age: 0,
                genome,
                brain: BrainState::default(),
                last_senses: [0.0; N_SENSES],
                last_turn: 0.0,
                last_speed: 0.0,
            });
        }

        Self {
            next_id: cfg.world.initial_organisms as u64,
            cfg,
            seed,
            tick: 0,
            rng,
            organisms,
            food,
            detritus: Vec::new(),
            light,
            ledger,
            influx_accum: 0.0,
            births: 0,
            deaths_starve: 0,
            deaths_age: 0,
            org_grid: Grid::default(),
            food_grid: Grid::default(),
            decisions: Vec::new(),
            order: Vec::new(),
        }
    }

    /// Advance the world by one tick.
    pub fn step(&mut self) {
        let cfg = self.cfg.clone();
        let (w, h) = (cfg.world.width, cfg.world.height);

        // 1. Sunlight grows food. Energy is injected only when a pellet
        // actually appears; light falling on a saturated world is lost.
        self.influx_accum += cfg.env.influx_per_tick;
        while self.influx_accum >= cfg.env.food_energy {
            if self.food.len() >= cfg.env.food_cap as usize {
                self.influx_accum %= cfg.env.food_energy;
                break;
            }
            let (x, y) = sample_light_pos(&mut self.rng, &self.light, w, h);
            self.food.push(Food {
                x,
                y,
                energy: cfg.env.food_energy,
            });
            self.ledger.injected += cfg.env.food_energy;
            self.influx_accum -= cfg.env.food_energy;
        }

        // 2. Rebuild spatial grids.
        self.rebuild_grids();

        // 3. Sense and think, all against the same pre-movement world.
        let n = self.organisms.len();
        self.decisions.clear();
        for i in 0..n {
            let senses = self.senses_of(i, &cfg);
            let o = &mut self.organisms[i];
            let decision = brain::think(&o.genome, &mut o.brain, &senses);
            o.last_senses = senses;
            self.decisions.push(decision);
        }

        // 4. Move and pay for it.
        let mut radiated = 0.0f64;
        for i in 0..n {
            let (turn, thrust) = self.decisions[i];
            let o = &mut self.organisms[i];
            o.heading = wrap_angle(o.heading + turn * cfg.body.turn_rate);
            let v = thrust * o.genome.body.max_speed;
            o.x = (o.x + v * libm::cosf(o.heading)).rem_euclid(w);
            o.y = (o.y + v * libm::sinf(o.heading)).rem_euclid(h);
            o.last_turn = turn;
            o.last_speed = v;
            let cost = cfg.body.move_cost * o.genome.body.size as f64 * (v as f64) * (v as f64);
            let burn = cost.min(o.energy);
            o.energy -= burn;
            radiated += burn;
        }

        // 5. Eat, in per-tick shuffled order. Food does not move, so the
        // pre-movement food grid stays valid; eaten pellets are skipped by
        // their zeroed energy and swept at the end of the tick.
        let mut order = core::mem::take(&mut self.order);
        order.clear();
        order.extend(0..n as u32);
        for k in (1..n).rev() {
            let j = self.rng.below(k + 1);
            order.swap(k, j);
        }
        for &oi in &order {
            let oi = oi as usize;
            let (ox, oy) = (self.organisms[oi].x, self.organisms[oi].y);
            let reach = self.organisms[oi].radius(&cfg) + cfg.body.food_radius;
            let capacity = self.organisms[oi].capacity(&cfg);
            let mut budget = self.organisms[oi].bite_budget(&cfg);
            self.food_grid.for_each_neighbor(ox, oy, |fi| {
                if budget <= 0.0 {
                    return;
                }
                let f = &mut self.food[fi as usize];
                if f.energy <= 0.0 {
                    return;
                }
                let dx = wrap_delta(ox, f.x, w);
                let dy = wrap_delta(oy, f.y, h);
                if dx * dx + dy * dy > reach * reach {
                    return;
                }
                let o = &mut self.organisms[oi];
                let bite = budget.min(f.energy).min(capacity - o.energy).max(0.0);
                f.energy -= bite;
                o.energy += bite;
                budget -= bite;
                // Crumbs too small to matter spoil to heat rather than
                // lingering as clutter.
                if f.energy > 0.0 && f.energy < 0.005 {
                    radiated += f.energy;
                    f.energy = 0.0;
                }
            });
        }

        // 6. Reproduce: mutate first, then pay the exact price of the child
        // that resulted. Children join the world this tick but are appended
        // after the loop, so they act for the first time next tick.
        for &oi in &order {
            let oi = oi as usize;
            let parent = &self.organisms[oi];
            let capacity = parent.capacity(&cfg);
            if parent.energy < cfg.repro.threshold_frac * capacity {
                continue;
            }
            let child_genome = parent.genome.mutated(&mut self.rng, &cfg);
            let child_soma = soma_for(child_genome.body.size, &cfg);
            let child_energy = cfg.repro.child_energy_frac
                * cfg.body.capacity_per_size
                * child_genome.body.size as f64;
            let cost = child_energy + child_soma + cfg.repro.overhead;
            let parent = &self.organisms[oi];
            if cost > parent.energy {
                continue; // the mutated child is unaffordable; no birth
            }
            let cx = (parent.x - cfg.repro.spawn_offset * libm::cosf(parent.heading)).rem_euclid(w);
            let cy = (parent.y - cfg.repro.spawn_offset * libm::sinf(parent.heading)).rem_euclid(h);
            let child_generation = parent.generation + 1;
            let heading = self.rng.range_f32(-PI, PI);
            self.organisms[oi].energy -= cost;
            radiated += cfg.repro.overhead;
            let child = Organism {
                id: self.next_id,
                generation: child_generation,
                x: cx,
                y: cy,
                heading,
                energy: child_energy,
                soma: child_soma,
                age: 0,
                genome: child_genome,
                brain: BrainState::default(),
                last_senses: [0.0; N_SENSES],
                last_turn: 0.0,
                last_speed: 0.0,
            };
            self.next_id += 1;
            self.births += 1;
            self.organisms.push(child);
        }
        self.order = order;

        // 7. Upkeep and aging — everyone, newborns included.
        for o in &mut self.organisms {
            let burn = o.upkeep(&cfg).min(o.energy);
            o.energy -= burn;
            radiated += burn;
            o.age += 1;
        }

        // 8. Death: out of energy, or out of time. The body — soma plus any
        // remaining energy — becomes detritus where it fell.
        let mut kept = 0usize;
        for i in 0..self.organisms.len() {
            let o = &self.organisms[i];
            let starved = o.energy <= 0.0;
            let aged_out = (o.age as f32) > o.genome.body.max_age;
            if starved || aged_out {
                if starved {
                    self.deaths_starve += 1;
                } else {
                    self.deaths_age += 1;
                }
                self.detritus.push(Detritus {
                    x: o.x,
                    y: o.y,
                    energy: o.energy.max(0.0) + o.soma,
                    ticks_left: cfg.env.compost_delay,
                });
            } else {
                self.organisms.swap(kept, i);
                kept += 1;
            }
        }
        self.organisms.truncate(kept);

        // 9. Compost: detritus decays back into food, imperfectly.
        for i in 0..self.detritus.len() {
            let d = &mut self.detritus[i];
            d.ticks_left -= 1;
            if d.ticks_left == 0 {
                let kept_e = d.energy * cfg.env.compost_efficiency;
                radiated += d.energy - kept_e;
                let (x, y) = (d.x, d.y);
                self.food.push(Food {
                    x,
                    y,
                    energy: kept_e,
                });
            }
        }
        self.detritus.retain(|d| d.ticks_left > 0);
        self.food.retain(|f| f.energy > 0.0);

        self.ledger.radiated += radiated;
        self.tick += 1;

        #[cfg(debug_assertions)]
        {
            let err = self.conservation_error();
            debug_assert!(
                err.abs() <= 1e-9 * self.ledger.injected.max(1.0),
                "energy leak at tick {}: {}",
                self.tick,
                err
            );
        }
    }

    pub(crate) fn rebuild_grids(&mut self) {
        let (w, h) = (self.cfg.world.width, self.cfg.world.height);
        let r = self.cfg.world.sense_radius;
        self.org_grid.reset(w, h, r);
        for (i, o) in self.organisms.iter().enumerate() {
            self.org_grid.insert(o.x, o.y, i as u32);
        }
        self.food_grid.reset(w, h, r);
        for (i, f) in self.food.iter().enumerate() {
            self.food_grid.insert(f.x, f.y, i as u32);
        }
    }

    fn senses_of(&mut self, i: usize, cfg: &Config) -> [f32; N_SENSES] {
        let (w, h) = (cfg.world.width, cfg.world.height);
        let r = cfg.world.sense_radius;
        let o = &self.organisms[i];

        let (mut food_dir, mut food_dist) = (0.0f32, 0.0f32);
        if let Some((fi, d2)) = self.nearest_food(o.x, o.y) {
            let f = &self.food[fi];
            let dx = wrap_delta(o.x, f.x, w);
            let dy = wrap_delta(o.y, f.y, h);
            food_dir = wrap_angle(libm::atan2f(dy, dx) - o.heading) / PI;
            food_dist = 1.0 - d2.sqrt() / r;
        }

        let (mut org_dir, mut org_dist) = (0.0f32, 0.0f32);
        if let Some((ni, d2)) = self.nearest_organism(o.x, o.y, i) {
            let other = &self.organisms[ni];
            let dx = wrap_delta(o.x, other.x, w);
            let dy = wrap_delta(o.y, other.y, h);
            org_dir = wrap_angle(libm::atan2f(dy, dx) - o.heading) / PI;
            org_dist = 1.0 - d2.sqrt() / r;
        }

        let energy = (o.energy / o.capacity(cfg)).clamp(0.0, 1.0) as f32;
        let age = (o.age as f32 / o.genome.body.max_age).min(1.0);
        let p = cfg.body.oscillator_period;
        let osc = libm::sinf(TAU * ((o.age as f32) % p) / p);
        let noise = self.rng.range_f32(-1.0, 1.0);

        [
            1.0, food_dir, food_dist, org_dir, org_dist, energy, age, noise, osc,
        ]
    }

    pub(crate) fn nearest_food(&self, x: f32, y: f32) -> Option<(usize, f32)> {
        let (w, h) = (self.cfg.world.width, self.cfg.world.height);
        let r2 = self.cfg.world.sense_radius * self.cfg.world.sense_radius;
        let mut best: Option<(usize, f32)> = None;
        self.food_grid.for_each_neighbor(x, y, |fi| {
            let f = &self.food[fi as usize];
            if f.energy <= 0.0 {
                return;
            }
            let dx = wrap_delta(x, f.x, w);
            let dy = wrap_delta(y, f.y, h);
            let d2 = dx * dx + dy * dy;
            if d2 <= r2 && best.is_none_or(|(_, b)| d2 < b) {
                best = Some((fi as usize, d2));
            }
        });
        best
    }

    pub(crate) fn nearest_organism(&self, x: f32, y: f32, exclude: usize) -> Option<(usize, f32)> {
        let (w, h) = (self.cfg.world.width, self.cfg.world.height);
        let r2 = self.cfg.world.sense_radius * self.cfg.world.sense_radius;
        let mut best: Option<(usize, f32)> = None;
        self.org_grid.for_each_neighbor(x, y, |oi| {
            if oi as usize == exclude {
                return;
            }
            let o = &self.organisms[oi as usize];
            let dx = wrap_delta(x, o.x, w);
            let dy = wrap_delta(y, o.y, h);
            let d2 = dx * dx + dy * dy;
            if d2 <= r2 && best.is_none_or(|(_, b)| d2 < b) {
                best = Some((oi as usize, d2));
            }
        });
        best
    }

    /// Total energy currently inside the world.
    pub fn world_energy(&self) -> f64 {
        let orgs: f64 = self.organisms.iter().map(|o| o.energy + o.soma).sum();
        let food: f64 = self.food.iter().map(|f| f.energy).sum();
        let det: f64 = self.detritus.iter().map(|d| d.energy).sum();
        orgs + food + det
    }

    /// How far the world is from perfect conservation. Should be ~0 always.
    pub fn conservation_error(&self) -> f64 {
        self.world_energy() + self.ledger.radiated - self.ledger.injected
    }

    /// Measure the world. Read-only: sampling draws from a throwaway RNG
    /// derived from (seed, tick), so observation can never perturb physics
    /// and the same tick always reports the same numbers.
    pub fn sample_metrics(&self) -> Metrics {
        Metrics::measure(self)
    }

    pub fn snapshot_bytes(&self) -> Vec<u8> {
        let mut out = SNAPSHOT_MAGIC.to_vec();
        out.extend(postcard::to_allocvec(self).expect("world serializes"));
        out
    }

    pub fn from_snapshot_bytes(bytes: &[u8]) -> Result<World, String> {
        if bytes.len() < SNAPSHOT_MAGIC.len() || &bytes[..SNAPSHOT_MAGIC.len()] != SNAPSHOT_MAGIC {
            return Err(format!(
                "not a vivere snapshot (expected magic {:?})",
                core::str::from_utf8(SNAPSHOT_MAGIC).unwrap()
            ));
        }
        postcard::from_bytes(&bytes[SNAPSHOT_MAGIC.len()..])
            .map_err(|e| format!("snapshot failed to decode: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spatial grid must agree exactly with a naive scan over all
    /// entities — same nearest neighbor, same distance.
    #[test]
    fn nearest_queries_match_naive_scan() {
        let mut world = World::new(Config::test_small(), 11);
        for _ in 0..60 {
            world.step();
        }
        world.rebuild_grids();
        let (w, h) = (world.cfg.world.width, world.cfg.world.height);
        let r2 = world.cfg.world.sense_radius * world.cfg.world.sense_radius;
        for i in 0..world.organisms.len() {
            let o = &world.organisms[i];
            let grid_food = world.nearest_food(o.x, o.y);
            let naive_food = world
                .food
                .iter()
                .enumerate()
                .filter_map(|(fi, f)| {
                    let dx = wrap_delta(o.x, f.x, w);
                    let dy = wrap_delta(o.y, f.y, h);
                    let d2 = dx * dx + dy * dy;
                    (d2 <= r2).then_some((fi, d2))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            assert_eq!(grid_food.map(|(_, d)| d), naive_food.map(|(_, d)| d));

            let grid_org = world.nearest_organism(o.x, o.y, i);
            let naive_org = world
                .organisms
                .iter()
                .enumerate()
                .filter(|(oi, _)| *oi != i)
                .filter_map(|(oi, other)| {
                    let dx = wrap_delta(o.x, other.x, w);
                    let dy = wrap_delta(o.y, other.y, h);
                    let d2 = dx * dx + dy * dy;
                    (d2 <= r2).then_some((oi, d2))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            assert_eq!(grid_org.map(|(_, d)| d), naive_org.map(|(_, d)| d));
        }
    }
}
