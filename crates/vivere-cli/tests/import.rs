//! The frozen v0.1 fixture must import cleanly forever: aligned decode
//! (guarded by walls + conservation checks inside the importer) and a
//! future that runs under v0.2 physics without leaking energy.

use vivere_cli::legacy_v01::import_v01;

const FIXTURE: &[u8] = include_bytes!("fixtures/v01_small.snap");

#[test]
fn v01_fixture_imports_and_runs_conserved() {
    let mut world = import_v01(FIXTURE, true).expect("fixture imports");
    assert_eq!(world.tick, 300);
    assert_eq!(world.organisms.len(), 148);
    assert!(world.cfg.contact.enabled);
    // The imported past is intact…
    assert!(world.births > 0);
    // …and the future is conserved under the new physics.
    for _ in 0..300 {
        world.step();
    }
    let err = world.conservation_error().abs();
    let bound = 1e-9 * world.ledger.injected.max(1.0);
    assert!(err <= bound, "drift {err} > {bound}");
}

#[test]
fn quarantined_import_never_bites() {
    let mut world = import_v01(FIXTURE, false).expect("fixture imports");
    assert!(!world.cfg.contact.enabled);
    for _ in 0..300 {
        world.step();
    }
    assert_eq!(world.bites, 0);
}

#[test]
fn garbage_is_refused() {
    assert!(import_v01(b"VIVERE02not actually v1", true).is_err());
    assert!(import_v01(b"", true).is_err());
}
