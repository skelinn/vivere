//! The brain: a tiny recurrent network compiled from connection genes.
//! Hidden neurons keep their activation across ticks (it is world state and
//! is serialized in snapshots); persistence and feedback exist only where a
//! genome wires them. All transcendentals go through `libm` so evaluation
//! is bit-identical across platforms.

use crate::genome::Genome;
use serde::{Deserialize, Serialize};

// v0.2 appended senses 9–11 and output 2; v0.3 grows the hidden pool 8→16
// (the v0.2 census found ~7 of 8 neurons alive in evolved brains — the pool
// was crowded). Appending preserves meaning: old wiring indices still point
// at the same neurons, and new capacity is reachable only through mutation.
pub const N_SENSES: usize = 12;
pub const N_HIDDEN: usize = 16;
pub const N_OUTPUTS: usize = 3;

pub const SENSE_NAMES: [&str; N_SENSES] = [
    "bias",
    "food_dir",
    "food_dist",
    "org_dir",
    "org_dist",
    "energy",
    "age",
    "noise",
    "osc",
    "org_size",
    "org_hue",
    "drain_felt",
];
pub const OUTPUT_NAMES: [&str; N_OUTPUTS] = ["turn", "thrust", "bite"];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct BrainState {
    pub hidden: [f32; N_HIDDEN],
}

/// One thought: senses in, `(turn ∈ [-1,1], thrust ∈ [0,1], bite ∈ [0,1])`
/// out.
///
/// Two passes: hidden sums read senses and the *previous* tick's hidden
/// activations (recurrence), then outputs read senses and the fresh hidden
/// activations. Thrust maps tanh to [0,1] with 0.5 at rest — an unwired
/// brain drifts forward at half throttle, which is enough for founders to
/// stumble onto food before any steering has evolved. Bite is continuous
/// *effort* with 0 at rest — the same shape as thrust, no threshold, no
/// deadzone: a brain with nothing wired to bite bites exactly zero, and
/// how hard to bite is as evolvable as whether.
pub fn think(genome: &Genome, state: &mut BrainState, senses: &[f32; N_SENSES]) -> (f32, f32, f32) {
    let mut hsum = [0.0f32; N_HIDDEN];
    for c in &genome.connections {
        if !c.to_output {
            let v = if c.from_hidden {
                state.hidden[c.from as usize]
            } else {
                senses[c.from as usize]
            };
            hsum[c.to as usize] += v * c.weight;
        }
    }
    let mut hidden = [0.0f32; N_HIDDEN];
    for (h, s) in hidden.iter_mut().zip(hsum) {
        *h = libm::tanhf(s);
    }

    let mut osum = [0.0f32; N_OUTPUTS];
    for c in &genome.connections {
        if c.to_output {
            let v = if c.from_hidden {
                hidden[c.from as usize]
            } else {
                senses[c.from as usize]
            };
            osum[c.to as usize] += v * c.weight;
        }
    }
    state.hidden = hidden;

    let turn = libm::tanhf(osum[0]);
    let thrust = 0.5 * (libm::tanhf(osum[1]) + 1.0);
    let bite = libm::tanhf(osum[2]).max(0.0);
    (turn, thrust, bite)
}
