//! Library surface of the CLI crate: importers and the genome census,
//! exposed so integration tests can exercise them against frozen fixtures.

pub mod inspect;
pub mod legacy_v01;

use vivere_core::World;

/// Decode any vivere snapshot by sniffing its magic: legacy formats route
/// through their frozen importers, the current format decodes directly.
/// `contact_for_v01` applies only when a v0.1 snapshot (which predates the
/// contact channel) is being imported.
pub fn load_any_snapshot(bytes: &[u8], contact_for_v01: bool) -> Result<World, String> {
    if bytes.starts_with(b"VIVERE01") {
        legacy_v01::import_v01(bytes, contact_for_v01)
    } else {
        World::from_snapshot_bytes(bytes)
    }
}
