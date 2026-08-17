//! The brain: a tiny recurrent network compiled from connection genes.
//! Hidden neurons keep their activation across ticks (it is world state and
//! is serialized in snapshots); persistence and feedback exist only where a
//! genome wires them. All transcendentals go through `libm` so evaluation
//! is bit-identical across platforms.

use crate::genome::Genome;
use serde::{Deserialize, Serialize};

pub const N_SENSES: usize = 9;
pub const N_HIDDEN: usize = 8;
pub const N_OUTPUTS: usize = 2;

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
];
pub const OUTPUT_NAMES: [&str; N_OUTPUTS] = ["turn", "thrust"];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct BrainState {
    pub hidden: [f32; N_HIDDEN],
}

/// One thought: senses in, `(turn ∈ [-1,1], thrust ∈ [0,1])` out.
///
/// Two passes: hidden sums read senses and the *previous* tick's hidden
/// activations (recurrence), then outputs read senses and the fresh hidden
/// activations. Thrust maps tanh to [0,1] with 0.5 at rest — an unwired
/// brain drifts forward at half throttle, which is enough for founders to
/// stumble onto food before any steering has evolved.
pub fn think(genome: &Genome, state: &mut BrainState, senses: &[f32; N_SENSES]) -> (f32, f32) {
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
    (turn, thrust)
}
