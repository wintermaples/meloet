//! Proves the watch itself works. Kept in its own test binary so its deliberate allocation does
//! not show up in the counts of the other tests.

mod common;

#[test]
fn allocation_inside_an_audio_block_is_counted() {
    let outside = vec![1u8; 64];
    drop(outside);
    assert_eq!(common::audio_block_counts(), (0, 0));

    {
        let _block = meloet_engine::rt::enter();
        let inside = std::hint::black_box(vec![1u8; 64]);
        drop(inside);
    }
    assert_eq!(common::audio_block_counts(), (1, 1));

    drop(vec![1u8; 64]);
    assert_eq!(common::audio_block_counts(), (1, 1));
}
