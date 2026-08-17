//! The three promises vivere-core makes, tested end to end:
//! determinism, energy conservation, and exact snapshot/restore.

use vivere_core::{Config, World};

fn run_bytes(seed: u64, ticks: u64) -> Vec<u8> {
    let mut w = World::new(Config::test_small(), seed);
    for _ in 0..ticks {
        w.step();
    }
    w.snapshot_bytes()
}

/// Same seed + same code ⇒ byte-identical world state.
#[test]
fn same_seed_same_world() {
    assert_eq!(run_bytes(42, 1500), run_bytes(42, 1500));
}

#[test]
fn different_seed_different_world() {
    assert_ne!(run_bytes(42, 300), run_bytes(43, 300));
}

/// Energy only enters via sunlight and only leaves as heat; the books
/// balance every tick.
#[test]
fn energy_is_conserved() {
    let mut w = World::new(Config::test_small(), 7);
    for _ in 0..4000 {
        w.step();
        let err = w.conservation_error().abs();
        let bound = 1e-9 * w.ledger.injected.max(1.0);
        assert!(err <= bound, "tick {}: drift {} > {}", w.tick, err, bound);
    }
    assert!(w.ledger.injected > 0.0);
}

/// A restored snapshot continues exactly as the original would have.
#[test]
fn snapshot_restore_is_exact() {
    let mut a = World::new(Config::test_small(), 42);
    for _ in 0..800 {
        a.step();
    }
    let snap = a.snapshot_bytes();
    let mut b = World::from_snapshot_bytes(&snap).expect("snapshot decodes");
    for _ in 0..400 {
        a.step();
        b.step();
    }
    assert_eq!(a.snapshot_bytes(), b.snapshot_bytes());
}

/// Observation must not perturb physics: sampling metrics mid-run leaves
/// the world byte-identical to an unobserved twin, and re-measuring the
/// same tick gives the same numbers.
#[test]
fn measuring_does_not_disturb() {
    let mut observed = World::new(Config::test_small(), 5);
    let mut unobserved = World::new(Config::test_small(), 5);
    for i in 0..600 {
        observed.step();
        unobserved.step();
        if i % 50 == 0 {
            let m1 = observed.sample_metrics();
            let m2 = observed.sample_metrics();
            assert_eq!(m1.diversity, m2.diversity);
        }
    }
    assert_eq!(observed.snapshot_bytes(), unobserved.snapshot_bytes());
}

/// Corrupt or foreign files are refused, not misread.
#[test]
fn snapshot_rejects_garbage() {
    assert!(World::from_snapshot_bytes(b"not a snapshot at all").is_err());
    assert!(World::from_snapshot_bytes(b"").is_err());
}
