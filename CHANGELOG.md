# Changelog

Behavior-affecting changes make old seeds produce new histories; each entry
notes the snapshot format it writes. Old snapshots are never silently
reinterpreted — formats are magic-tagged and refused on mismatch.

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
