//! The samples that were let go are counted, and their memory can be given back
//! (docs/architecture.md 23-12). A file of its own: the count is one for the process.

mod support;

use std::sync::Arc;

use meloet_sound::memory::{freed_bytes, give_back, malloc_counts};
use meloet_sound::sampler::load_instrument;
use support::pack::{TestPack, region, tone};

#[test]
fn a_bank_counts_as_freed_when_its_last_holder_lets_go() {
    let pack = TestPack::new("memory");
    pack.wav("mono", 48_000, &[&tone(440.0, 48_000, 48_000, 0.5)]);
    let json = pack.instrument("", &[region("mono", 0, 127, 60, "")]);
    let bank = load_instrument(&json, 1).unwrap();
    let bytes = bank.sample_bytes();
    assert_eq!(bytes, 48_000 * 2);

    give_back();
    let held = Arc::clone(&bank);
    drop(bank);
    assert_eq!(freed_bytes(), 0, "another holder still has it");
    drop(held);
    assert_eq!(freed_bytes(), bytes);

    give_back();
    assert_eq!(freed_bytes(), 0);
    assert!(malloc_counts().is_some_and(|m| m.in_use > 0));
}
