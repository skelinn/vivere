# Contributing to vivere

Thanks for helping build a world where life can arise. This project has an
unusual constraint: the two invariants below outrank features, performance,
and elegance. PRs that break them don't merge; PRs that strengthen them are
the most valuable kind.

## Setup

```sh
# rustup pins the toolchain via rust-toolchain.toml automatically
make build     # build everything
make test      # determinism, conservation, snapshot exactness + unit tests
make lint      # cargo fmt --check && clippy -D warnings
make run       # open the viewer
make sim SEED=42 TICKS=50000 OUT=runs/x.csv   # headless run
```

CI runs `make lint` and `make test` on every push. Both must be green.

## Invariant 1 — energy is conserved

`world_energy() + radiated == injected`, always (debug builds assert it
every tick).

- Every energy flow is an **exact pair transfer**: the amount leaving one
  pool lands in another pool or in `ledger.radiated`, in the same statement.
- Never create energy ("spawn a pellet") without `ledger.injected += …`;
  never destroy it without `radiated += …`.
- Costs that can't be fully paid charge `min(cost, energy)` — sinks may
  clamp, transfers may not (a partial transfer must stay exact on both
  sides).
- If you add a mechanic, extend `world_energy()` to count any new pool, and
  make sure `energy_is_conserved` still passes at 1e-9 relative.

## Invariant 2 — runs are deterministic

Same seed + same commit = same run, byte for byte (`same_seed_same_world`
test).

Checklist for any change touching the sim path (`vivere-core`):

- [ ] No `HashMap`/`HashSet` iteration (use `Vec`s; the grids exist for
      neighbor queries).
- [ ] All randomness comes from `world.rng` (physics) or the per-tick
      metrics stream — never `rand`, never the clock, never addresses.
- [ ] RNG draws happen in a fixed, documented order; remember that adding a
      draw changes every subsequent number (that's fine — it's a new
      commit — but never let draw *order* depend on float comparisons with
      ties or on container iteration order).
- [ ] Transcendentals via `libm` only (`std` `sqrt` is fine; it's
      IEEE-exact). No `fastmath`, no SIMD reductions with reordered sums.
- [ ] Simulation stays single-threaded for now (see DESIGN.md for the
      approved future parallelism pattern).
- [ ] New world state is serialized; new transient caches are
      `#[serde(skip)]` and rebuilt. If you changed the snapshot layout,
      bump `SNAPSHOT_MAGIC` (`VIVERE01` → `VIVERE02`) and say so in the PR.
- [ ] `snapshot_restore_is_exact` and `measuring_does_not_disturb` pass.

A behavior-affecting change (physics, constants, RNG draw order) makes old
seeds produce new histories. That's expected — note it in the PR
description so nobody chases a "regression" that is just a new universe.

## The doctrine test for proposals

Before proposing a feature, ask: **is it physics, or is it a script?**

- Physics: a new channel with a price (a resource, a force, a sense, a
  cost). Organisms may exploit it, ignore it, or die by it. ✅
- Script: a behavior, goal, score, or protection implemented *for*
  organisms (a fitness bonus, a "flee" routine, a spawn floor). ❌ — even
  when it would make the world "work better". Especially then.

Environment tuning PRs may touch `EnvCfg` (and initial stocks) only.
Body costs, reproduction constants, and mutation rates are frozen physics —
changing them is a *world-version* discussion, not a tuning fix.

## Style

- `cargo fmt` formats; clippy at `-D warnings` gates.
- Comments explain constraints and intent, not mechanics. The doc comments
  at the top of each module are the spec — keep them true.
- Keep `vivere-core` dependency-free beyond serde/postcard/libm. The
  viewer may not reach into sim internals to *change* anything: it reads.

## Reporting interesting worlds

A reproducible observation is `(commit, seed, tick range)` or a `.snap`
file. Attach the metrics CSV if you have it. "Weird thing at seed 1723
around tick 40k" with a snapshot is a great issue; a description without a
seed is a campfire story.
