# vivere — design

This document explains how v0.1 ("protocell") is built and, more importantly,
how it is meant to grow. The architecture optimizes for two invariants —
**energy conservation** and **determinism** — because they are what keep an
artificial-life project honest as it scales.

## Crate layout

```
vivere-core     the simulation: world, organisms, genomes, metrics, snapshots
vivere-cli      headless runner (binary: `vivere`): long runs, CSV, snapshots
vivere-viewer   macroquad window: watch, pause/step/speed, click-to-inspect
```

Rules: `vivere-core` has no graphics, windowing, threading, clock, or RNG
dependencies (serde + postcard + libm only). The viewer and CLI both drive
the same `World::step`; the viewer consumes no simulation randomness, so
watching a world and running it headless produce identical histories.

## The tick

Fixed phase order, one tick (`world.rs`):

1. **Sunlight** — an influx budget accumulates; while it covers a pellet and
   the world is below the food cap, a pellet spawns at a light-weighted
   location (rejection sampling against a static light field: ambient base +
   seed-placed Gaussian patches). Energy is *injected* only when a pellet
   materializes; light falling on a saturated world is lost, which gives
   logistic saturation with no special-case code.
2. **Grids** — dense uniform grids (organisms, food) rebuild. Cell ≥ sense
   radius, so any query is a 3×3 scan. Plain `Vec` buckets in insertion
   order: no hashing, no iteration-order nondeterminism, allocations reused.
3. **Sense + think** — every organism reads the same pre-movement world:
   nearest food (direction, proximity), nearest organism, own energy, age,
   noise, oscillator, bias. The brain (below) returns turn and thrust.
4. **Move** — heading and position update; movement costs
   `move_cost · size · v²`, radiated as heat.
5. **Eat** — automatic on contact, bite-limited per tick
   (`bite_rate · metab · size`), capped by pellet content and free capacity;
   what isn't taken stays in the pellet. Contention resolves in a fresh
   random permutation each tick, so no lineage gets a standing index
   advantage.
6. **Reproduce** — automatic above `0.75 · capacity`. The genome is mutated
   *first*, then the parent pays the exact price of the child that resulted
   (child's starting energy + child's soma + fixed overhead → heat). If a
   mutation made the child unaffordable, no birth.
7. **Upkeep + aging** — basal burn: a size-independent floor (being alive
   costs something, so miniaturization isn't free), plus terms for
   size×metabolism, size×longevity, and brain connections (computation
   costs energy — genome growth must earn its keep).
8. **Death** — energy ≤ 0 or age > max_age. Nothing else kills. The corpse
   (soma + leftover energy) becomes detritus where it fell.
9. **Compost** — after a delay, detritus becomes food at an efficiency < 1;
   the difference radiates. The cap from (1) does not gate compost — decay
   is not photosynthesis.

## Energy ledger

Two counters: `injected` (all energy that ever entered: initial world
contents + grown food) and `radiated` (all energy that ever left: metabolic
heat, birth overhead, compost losses, crumb spoilage). Invariant, asserted
every tick in debug builds and by tests:

```
Σ organisms (energy + soma) + Σ food + Σ detritus + radiated == injected
```

Every flow is an exact pair transfer — a quantity leaves one pool and lands
in another (or in `radiated`) in the same statement. There is no "clamp and
forget": costs that can't be fully paid charge only what exists. This is
what makes the conservation test tight (~1e-12 relative drift, pure f64
rounding) rather than "approximately balanced".

Soma is the structural energy of a body: paid by the parent at birth,
inert during life, returned as detritus at death. It exists so that a
starved corpse still feeds the detritus cycle, and so that making a body
costs more than filling its tank.

## Brain and genome

biosim4-style: the genome is a flat list of connection genes
`(source, sink, weight)` where sources are senses or hidden neurons and
sinks are hidden neurons or outputs. Fixed pools: 9 senses, 8 hidden, 2
outputs (turn, thrust). Hidden activations persist across ticks — they are
world state, serialized in snapshots — so recurrence and memory exist
exactly where a genome wires them. Duplicate connections sum, which is what
makes gene duplication a real mutation rather than a no-op.

Body genes — size, max speed, metabolic rate, max age, hue — map to costs
and capacities in `BodyCfg` (capacity and soma scale with size; bites scale
with metab×size; upkeep as above). Hue has no physics: it's a neutral
marker that makes lineages visible.

Mutation (per child): per-gene weight perturbation and rare rewiring;
per-child connection add / remove / duplicate; per-gene body perturbation.
Rates live in `MutationCfg` and are **frozen heredity physics** — they are
not tuned to make outcomes nicer, and they are not environment knobs.

## The tuning contract

When a world collapses or explodes, we change the climate, not the
organisms. Tunable: `EnvCfg` only — influx rate, pellet energy, food cap,
light shape, compost delay/efficiency (plus initial stocks in `WorldCfg`).
Frozen: every body cost, reproduction constant, and mutation rate.
`config.rs` documents which is which, and the doctrine is enforced by
review, not code — contributors, hold the line.

## Determinism

The promise: same seed + same commit = same run, byte for byte
(`snapshot_bytes()` equality, tested).

- Single-threaded simulation. No `HashMap`/`HashSet` anywhere in the sim
  path; dense grids with stable insertion order.
- vivere owns its RNG (xoshiro256++, serialized state, in-repo). Gaussians
  are Box–Muller *without* the cached spare (hidden state would break
  snapshot equality). Two independent streams: physics, and a throwaway
  per-tick stream for metrics sampling — so observation cadence can never
  perturb a run (tested).
- All transcendentals (`sin`, `cos`, `atan2`, `tanh`, `exp`, `log`) go
  through `libm`, which is bit-stable across platforms; `sqrt` is IEEE-exact
  everywhere. Angles wrap each tick so trig arguments stay small.
- Energy is f64 end to end; positions are f32.
- The toolchain is pinned (`rust-toolchain.toml`) — the compiler is part of
  the commit.
- Caveat: we *design* for cross-platform bit-equality but currently *test*
  same-platform only.

**Keeping it under future parallelism**: determinism dies from reduction
order, not concurrency per se. The path (when profiling demands it) is:
partition space into tiles, compute intents in parallel, merge in fixed
tile order — never atomics-into-shared-floats, never "first thread wins".
The phase structure above was chosen so each phase is either
embarrassingly parallel over organisms (sense/think, move, upkeep) or a
cheap ordered commit (eat, reproduce, death).

## Snapshots

`postcard` serialization of the whole `World` — config, RNG state, ledger,
every organism including hidden activations — behind an 8-byte magic
(`VIVERE01`; bump on format change, refuse mismatches). Restore is exact:
a restored world's future is byte-identical to the original's (tested).
Transient caches (grids, scratch buffers, inspector values) are
`serde(skip)` and rebuilt on demand. Snapshots are how long runs resume
and how interesting moments get shared — a `.snap` file **is** a
reproducible moment.

## Instrumentation

`Metrics::measure` is read-only and cadence-independent. The diversity
metric samples ≤60 organisms per measurement (seeded by `(seed, tick)`) and
averages pairwise genome distance: matched connections compare weights,
unmatched count fully, body genes compare normalized (hue circularly).
CSV writing is hand-rolled in the CLI; the header lives next to the row
formatter in `metrics.rs`.

## How each layer is meant to grow

The guiding rule for all of these: **grow the world, then let organisms
discover it.** No new verbs for organisms — new physics with consequences.

- **Chemistry (multiple resource types)** — `Food { energy }` becomes a
  vector of compounds; metabolism becomes genome-encoded conversion
  pathways with conserved totals per element. The ledger generalizes from
  one conserved scalar to a conserved vector. Eating, detritus, and light
  already flow through single points in `world.rs`, which is where the
  vector swap lands.
- **Contact / predation** — a physical-overlap phase between move and eat:
  bodies become resources (soma is already on the books, so a corpse or a
  bitten body has well-defined energy). No `attack()` verb — reachability
  plus economics, and let hunting, fleeing, and armor be discovered.
- **Sexual reproduction** — reproduction is a single phase; crossover slots
  in as a second parent lookup (the organism-direction sense already
  exists). Speciation metrics fall out of the existing distance function.
- **Multicellularity** — `Organism` becomes a body of cells sharing a
  genome with per-cell expression; the sense→think→act loop runs per body,
  physics per cell. Soma generalizes to construction cost of a body plan.
- **GPU compute** — the data layout migrates to struct-of-arrays behind the
  same phase pipeline; each phase is already data-parallel with ordered
  commits (see determinism note). Fixed merge order is the non-negotiable.
- **WASM / browser** — `vivere-core` is already `no-graphics`, clock-free,
  and single-threaded: it compiles to `wasm32` as-is. macroquad has a web
  target for the viewer; the CLI stays native.
- **Alternate substrates (e.g., Lenia-like continuous CA)** — the seam is
  the sense→think→act contract between organisms and world: senses in,
  intents out, world applies physics. A continuous-field substrate would
  implement the same contract with different physics (fields instead of
  particles, morphogenesis instead of rigid bodies). Deliberately **not**
  abstracted yet: the core stays concrete until a second substrate exists
  (rule of two) — but nothing outside `world.rs` may assume particles, so
  the seam stays clean.
