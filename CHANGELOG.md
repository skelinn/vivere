# Changelog

Behavior-affecting changes make old seeds produce new histories; each entry
notes the snapshot format it writes. Old snapshots are never silently
reinterpreted — formats are magic-tagged and refused on mismatch.

## 0.3.0 — "cortex" (2026-08-17) — snapshot format `VIVERE03`

Mind size stops being an administrative wall and becomes an economic
frontier. Before changing anything we audited the evolved brains
(`vivere inspect`, new): both v0.2 contact worlds were ~90% live wiring
with 7 of 8 hidden neurons in use — the 64-connection cap was binding
real cognition, not mutation-ratchet junk.

- **Wiring cost curve**: brain upkeep is now `a·n + b·n²` (knee at n = 96,
  pre-registered before the experiments ran). Connectivity costs
  superlinearly, so bigger minds must be earned by income.
- **genome_cap 64 → 4096**: purely a runaway guard, ~4× beyond the apex
  economic ceiling. Standing rule: if max genome length exceeds 0.8 × cap,
  the cap rises next release; the curve constant is never touched.
- **Hidden pool 8 → 16** (measured crowded), appended so evolved wiring
  keeps its meaning.
- **`vivere inspect`**: genome census of any snapshot — live/weak/dead
  wiring, hidden-pool utilization, stacked-weight analysis, and the
  per-sense table of what evolution actually reads.
- **Importer chain**: magic-sniffing `vivere import` (any older format);
  each legacy module owns its format's frozen shapes and converts forward,
  so every future bump touches exactly one writer. `run --resume` sniffs
  legacy formats too, and `--config` at resume explicitly replaces the
  embedded config for controlled A/B physics runs.
- **Metrics**: std/max genome length, mean hidden neurons alive,
  corr_brain_gain (brain size × bite income), live_conn_frac (the mid-run
  minds-vs-junk discriminator); progress line reports ticks/s.

Results (experiments A/A0/B/C in the release notes): the 9,000-generation
teeth world's minds grew 64 → ~166 connections under the curve (~212
without it — the curve holds back ~25%), reaching 95.7% live wiring and
colonizing all eight new hidden neurons to 100% fixation. Fresh worlds run
on ~30 connections; the v0.2-economics control re-pinned at the old cap,
replicating v0.2 exactly.

## 0.2.0 — "contact" (2026-08-17) — snapshot format `VIVERE02`

The world grows a physical channel between bodies; nothing is scripted
about how it gets used.

- **Contact physics**: a touch phase between move and eat. Bite is the
  brain's third output — continuous effort, zero at rest, no threshold.
  Firing costs a lunge (contact or not); landing drains the nearest
  touching victim inside a ±90° mouth cone at 75% transfer efficiency.
  Kills leave corpses to the existing detritus→compost cycle. A
  `contact.enabled` physics flag supports A/B worlds.
- **Three appended senses**: relative neighbor size, signed circular hue
  difference (kin discrimination/mimicry channel), and `drain_felt`
  interoception. Appended, so evolved v0.1 wiring keeps its meaning.
- **Wider gene walls** (~2.5×: size 0.5–4, speed 0.3–6, metab 0.25–3,
  max_age 500–15k) after v0.1 populations pinned 4 of 5 genes. Speed
  capacity now carries idle upkeep — unused muscle decays out. Longevity
  upkeep repriced to absolute ticks (wall positions are no longer
  load-bearing physics).
- **Log-space heredity** for scale genes: multiplicative mutation
  reflecting at the walls, log-uniform founders, log-space distance and
  spread metrics.
- **Metrics**: deaths_predation (attributed kills), bites, predation_flux,
  mean_bite_hue_dist with its ambient mean_neighbor_hue_dist control,
  std_size/std_speed (log-space bimodality watch).
- **`vivere import-v01`**: carries v0.1 snapshots into v0.2 physics
  (quarantined or with contact on), guarded by gene-wall and
  energy-conservation checksums. `run --override-contact` stages
  experiments on existing worlds.

## 0.1.0 — "protocell" (2026-08-17) — snapshot format `VIVERE01`

The first world: conserved-energy 2D ecosystem, evolvable neural steering,
asexual heredity with point mutation / connection add-remove / gene
duplication, reflexive eating and division, detritus→compost recycling,
deterministic byte-for-byte with exact snapshot/restore, CSV metrics with
observation-independent diversity sampling, macroquad viewer, headless GIF
renderer. Ran 1M ticks (~594 generations) on defaults untouched and
produced a full r→K strategy reversal.
