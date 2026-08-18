//! Library surface of the CLI crate: importers and the genome census,
//! exposed so integration tests can exercise them against frozen fixtures.

pub mod inspect;
pub mod legacy_v01;
pub mod legacy_v02;

use vivere_core::World;

/// Decode any vivere snapshot by sniffing its magic: legacy formats route
/// through their frozen importer chain, the current format decodes
/// directly. `contact_for_v01` applies only to v0.1 snapshots, which
/// predate the contact channel.
pub fn load_any_snapshot(bytes: &[u8], contact_for_v01: bool) -> Result<World, String> {
    if bytes.starts_with(b"VIVERE01") {
        legacy_v01::import_v01(bytes, contact_for_v01)
    } else if bytes.starts_with(b"VIVERE02") {
        legacy_v02::import_v02(bytes)
    } else {
        World::from_snapshot_bytes(bytes)
    }
}
