//! The energy ledger. vivere's honesty mechanism: energy enters the world
//! only by being `injected` (sunlight growing food, plus whatever the world
//! started with) and leaves only by being `radiated` (metabolic heat, birth
//! overhead, compost losses). At every tick:
//!
//! ```text
//! world_energy() + radiated == injected          (within f64 rounding)
//! ```
//!
//! where `world_energy` sums organism energy + soma, food, and detritus.
//! Every flow in `world.rs` is an exact pair transfer — the amount removed
//! from one pool is exactly the amount added to another (or to `radiated`)
//! — so conservation holds to floating-point rounding, and the debug build
//! asserts it every tick.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    /// Total energy that has ever entered the world.
    pub injected: f64,
    /// Total energy that has ever left the world as heat.
    pub radiated: f64,
}
