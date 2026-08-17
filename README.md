# vivere

[![ci](https://github.com/skelinn/vivere/actions/workflows/ci.yml/badge.svg)](https://github.com/skelinn/vivere/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![deterministic](https://img.shields.io/badge/runs-deterministic-8A2BE2)](docs/DESIGN.md#determinism)

> *vivere* (Latin): to live.

**vivere is an attempt to create life digitally — not to imitate it, but to build worlds where it can arise. We start with physics that conserves energy, add heredity and mutation, and let selection do the rest.**

Imitation is the trap this project is built to avoid. A simulated ant that follows an ant-behavior script is a puppet; nothing about it is alive, and nothing about it can surprise you. vivere goes the other way: build a small universe with honest physics — energy that is never created or destroyed, bodies that cost what they're made of, inheritance that errs — and then get out of the way. If something in that world feeds itself, outruns its costs, and leaves descendants, it is doing so because the world permits it, not because we told it to. The organisms owe us nothing, and that's the point: whatever survives is *real* survival, whatever evolves is *real* evolution, discovered rather than designed.

![protocell](docs/assets/protocell.gif)
*(GIF coming soon — run `make run` to watch a world live.)*

## Principles

- **Nothing is scripted.** No fitness function, no hand-written behaviors, no spawn quotas keeping populations comfortable. Organisms live or die by what the world does to them.
- **Energy is conserved.** It enters only as sunlight growing food, leaves only as metabolic heat, and the books are audited every tick (`world_energy + radiated == injected`, asserted in tests and debug builds). Conservation is what makes selection honest: every ability has a price, so nothing evolves for free.
- **Bodies are tradeoffs, not stats.** Bigger stores more energy but burns more; speed costs quadratically; longevity and brains have upkeep. The genome chooses a point in that space and the world grades the choice.
- **Determinism is a feature.** Same seed + same commit = same run, byte for byte. Every interesting moment is reproducible, sharable as a snapshot, and debuggable.
- **The instrument panel matters as much as the world.** Evolution you can't measure is an anecdote. Every run logs population, energy, genome size, diversity, and trait drift to CSV.

## Why not just add features?

Because features are ceilings and physics are floors. Every behavior we hand-write is a behavior that can never evolve, surprise us, or be selected against — a hard-coded "flee predators" routine forecloses the discovery of hiding, herding, armor, or bluffing. Every unconserved resource is a subsidy with no price, and selection can't price what's free. The discipline of this project is to grow the *world* — richer chemistry, more physical channels for interaction — and let organisms discover what those channels are for. When vivere gains predation, it won't be an `attack()` feature; it will be physics that makes other organisms' energy reachable, and whatever follows from that.

## v0.1 — "protocell"

A 2D continuous wraparound world. Uneven sunlight grows food; organisms sense (nearest food and neighbor, own energy, age, noise, an oscillator), think with a small evolvable recurrent net (biosim4-style connection genes + body genes for size, speed, metabolism, lifespan, color), and act (turn, thrust). Eating on contact and division above an energy threshold are reflexes — protocells don't decide. Corpses become detritus, which composts back into food. Death comes from starvation or old age, nothing else. Reproduction is asexual with point mutations, connection add/remove, and gene duplication.

In v0.1 evolution has exactly two things to work with: steering (the brain) and body-plan economics (the genes). That's deliberate — a small, closed, measurable loop, honest all the way down.

## Quickstart

```sh
git clone https://github.com/skelinn/vivere
cd vivere

# native viewer (pause/step/speed, click an organism to inspect it)
make run

# headless run: 50k ticks, metrics every 100 ticks
make sim SEED=42 TICKS=50000 OUT=runs/seed42.csv

# the CLI directly
cargo run --release -p vivere-cli -- run --seed 42 --ticks 100000 --out run.csv
cargo run --release -p vivere-cli -- run --seed 42 --ticks 50000 --save-snapshot runs/w.snap
cargo run --release -p vivere-cli -- run --resume runs/w.snap --ticks 50000   # continue
cargo run --release -p vivere-cli -- default-config > my.toml                 # edit, then --config my.toml

make test   # determinism, conservation, snapshot exactness
make lint   # fmt + clippy
```

Viewer controls: `space` pause · `.` step · `+`/`-` speed · mouse wheel zoom · right-drag pan · click inspect · `R` refit · `esc` deselect.

## Reading a run

The CSV logs, per window: `population`, `births`, `deaths_starve`/`deaths_age`, `mean_energy`/`max_energy`, `mean_genome_len`, `diversity` (mean pairwise genome distance over a sample), body-gene means (`mean_size`, `mean_speed_gene`, `mean_metab`, `mean_max_age`), `mean_actual_speed`, `mean_generation`, `food_count`, `detritus_count`, and the energy ledger (`world_energy`, `injected`, `radiated`, `drift` — drift should be ~1e-12 of total, i.e., zero).

Expectation-setting: a 50k-tick run is *early* evolution — some tens of generations, with selection acting only on foraging and body economics (there is no predation channel yet). Boom-and-bust population dynamics are normal for a fresh biosphere; trait drift may be subtle. That's a finding, not a bug.

## Roadmap

Each step grows the world, not the feature list. Order is intent, not promise.

- **v0.1 — protocell** (this): conserved energy, evolvable steering, asexual heredity, full instrumentation.
- **v0.2 — chemistry**: multiple resource types and conversion pathways; metabolism becomes something evolution can restructure.
- **v0.3 — contact**: organisms become physical to each other — energy in other bodies becomes reachable (predation, scavenging, and defense follow from physics, not features), plus sexual reproduction.
- **v0.4 — multicellularity**: bodies as cell collectives; development as part of the genome.
- **Beyond**: GPU compute for 10⁶-organism worlds, WASM/browser builds, and alternate substrates (e.g., continuous-CA worlds à la Lenia) behind the same sense→think→act seam — see [docs/DESIGN.md](docs/DESIGN.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The two house rules: keep energy conserved (every joule traceable) and keep runs deterministic (no unseeded randomness, no iteration-order surprises). Feature proposals get one question: *is it physics, or is it a script?*

## License

[MIT](LICENSE).
