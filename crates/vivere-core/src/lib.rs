//! vivere-core: the deterministic, conserved-energy world.
//!
//! No graphics, no clocks, no external randomness. Everything the simulation
//! does follows from a [`Config`] and a seed — same seed, same commit, same
//! run, byte for byte. Energy enters only as sunlight growing food, leaves
//! only as metabolic heat, and is audited by a ledger every tick.

pub mod brain;
pub mod config;
pub mod energy;
pub mod genome;
pub mod grid;
pub mod metrics;
pub mod organism;
pub mod rng;
pub mod world;

pub use config::Config;
pub use metrics::Metrics;
pub use world::World;
