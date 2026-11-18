use super::*;

/// xorshift64*, so the test needs nothing outside std.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn lens<T>(v: &ChunkedVec<T>) -> Vec<usize> {
    v.chunk_lens().collect()
}

fn shared<T>(a: &ChunkedVec<T>, b: &ChunkedVec<T>) -> usize {
    a.chunks()
        .filter(|c| b.chunks().any(|o| Arc::ptr_eq(&o.items, &c.items)))
        .count()
}

#[test]
fn empty_and_small() {
    let e: ChunkedVec<u64> = Vec::new().into();
    assert!(e.is_empty());
    assert_eq!(e.get(0), None);
    assert_eq!(e.first(), None);
    assert_eq!(e.iter().next(), None);
    assert_eq!(e, ChunkedVec::default());
    let s: ChunkedVec<u64> = vec![3, 1, 2].into();
    assert_eq!(s.len(), 3);
    assert_eq!(s[1], 1);
    assert_eq!(s, [3, 1, 2]);
    assert_eq!(s.iter().rev().copied().collect::<Vec<_>>(), [2, 1, 3]);
    assert_eq!(lens(&s), [3]);
}

#[test]
fn index_iter_and_both_ends_match_the_vec() {
    let v: Vec<u64> = (0..5000).map(|i| i * 7).collect();
    let c = ChunkedVec::from(v.clone());
    assert!(c.chunks().count() > 10, "{:?}", lens(&c));
    assert!(lens(&c).iter().all(|&n| n <= MAX));
    assert!(lens(&c).iter().rev().skip(1).all(|&n| n >= MIN));
    for (i, x) in v.iter().enumerate() {
        assert_eq!(c.get(i), Some(x));
    }
    assert_eq!(c.get(v.len()), None);
    assert_eq!(c.to_vec(), v);
    assert_eq!(c.iter().len(), v.len());
    assert!(c.iter().rev().eq(v.iter().rev()));
    // Both ends at once meet in the middle.
    let mut it = c.iter();
    let mut out = Vec::new();
    let mut back = Vec::new();
    while let Some(x) = it.next() {
        out.push(*x);
        match it.next_back() {
            Some(x) => back.push(*x),
            None => break,
        }
    }
    back.reverse();
    out.extend(back);
    assert_eq!(out, v);
    for x in [0, 1, 6, 7, 700, 6999, 7000, 34_993, 34_994, 35_000, 99_999] {
        assert_eq!(
            c.partition_point(|&y| y < x),
            v.partition_point(|&y| y < x),
            "{x}"
        );
    }
    assert_eq!(ChunkedVec::<u64>::default().partition_point(|_| true), 0);
    assert_eq!(c.first(), v.first());
    assert_eq!(c.last(), v.last());
}

/// Random inserts, removes, changes and moves on a plain `Vec`, each step shared with the
/// last: the content is always the `Vec`, cut the same way as when made from nothing, and a
/// change of one item leaves most chunks shared.
#[test]
fn shared_lists_hold_what_a_fresh_copy_holds() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut v: Vec<u64> = (0..6000).map(|_| rng.below(1 << 40)).collect();
    let mut c = ChunkedVec::from(v.clone());
    let mut new_chunks = 0;
    for round in 0..2000 {
        let n = v.len() as u64;
        match rng.below(5) {
            0 => v.insert(rng.below(n + 1) as usize, rng.below(1 << 40)),
            1 if n > 0 => {
                v.remove(rng.below(n) as usize);
            }
            2 if n > 0 => v[rng.below(n) as usize] = rng.below(1 << 40),
            3 if n > 0 => {
                let x = v.remove(rng.below(n) as usize);
                v.insert(rng.below(n) as usize, x);
            }
            _ => {
                // Many apart at once.
                for _ in 0..rng.below(20) {
                    let i = rng.below(v.len() as u64) as usize;
                    v[i] ^= 1;
                }
            }
        }
        let next = ChunkedVec::sharing(&v, &c);
        let fresh = ChunkedVec::from(v.clone());
        assert_eq!(next.to_vec(), v, "round {round}");
        assert_eq!(next, fresh, "round {round}");
        assert_eq!(lens(&next), lens(&fresh), "round {round}");
        for (a, b) in next.chunks().zip(fresh.chunks()) {
            assert_eq!((a.at, a.hash), (b.at, b.hash), "round {round}");
        }
        new_chunks += next.chunks().count() - shared(&next, &c);
        c = next;
    }
    // A change of one item makes 1 or 2 new chunks (3 when it moves far).
    assert!(new_chunks < 2000 * 4, "{new_chunks}");
}

#[test]
fn bytes_not_in_counts_only_the_new_chunks() {
    let v: Vec<u64> = (0..10_000).collect();
    let a = ChunkedVec::from(v.clone());
    let mut w = v.clone();
    w[5000] += 1;
    let b = ChunkedVec::sharing(&w, &a);
    assert_eq!(a.chunks().count() - shared(&b, &a), 1);
    let one = b.bytes_not_in(&a);
    assert!(
        one < (MAX + 64) * 8 + size_of_val::<[Chunk<u64>]>(&b.rest),
        "{one}"
    );
    assert_eq!(
        a.bytes_not_in(&ChunkedVec::default()),
        size_of_val::<[Chunk<u64>]>(&a.rest) + a.chunks().count() * 16 + v.len() * 8
    );
}

#[test]
fn the_same_run_is_cut_the_same_way_wherever_it_is() {
    let mut rng = Rng(7);
    let tail: Vec<u64> = (0..3000).map(|_| rng.next()).collect();
    let a = ChunkedVec::from(tail.clone());
    let mut longer: Vec<u64> = (0..37).map(|_| rng.next()).collect();
    longer.extend(&tail);
    let b = ChunkedVec::sharing(&longer, &a);
    // All but the first chunk or two of `a` are found again after the new head.
    assert!(
        shared(&b, &a) + 2 >= a.chunks().count(),
        "{:?} {:?}",
        lens(&a),
        lens(&b)
    );
}
