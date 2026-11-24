//! Decoded tiles shared across renders, and parallel decoding.
//!
//! A plot redrawn on resize renders the same tiles again: decoding them
//! (protobuf for vector tiles, PNG/JPEG/WebP for raster tiles) is kept per
//! process, keyed by the tile bytes themselves, so a tile is decoded once
//! however many times, and from however many sources, it is drawn. Misses
//! are decoded on all cores; results keep their input order, so renders
//! stay deterministic.

use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

/// A tile's identity: two independent 64-bit hashes of its bytes and its
/// length (a collision would need both hashes to collide at once).
type Key = (u64, u64, usize);

fn key(bytes: &[u8]) -> Key {
    let mut a = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut a);
    let mut b = std::collections::hash_map::DefaultHasher::new();
    0x9e37_79b9_7f4a_7c15_u64.hash(&mut b);
    bytes.hash(&mut b);
    (a.finish(), b.finish(), bytes.len())
}

/// A least-recently-used cache of decoded values, bounded by their
/// estimated size in bytes.
pub struct Memo<T> {
    entries: lru::LruCache<Key, (Arc<T>, usize)>,
    weight: usize,
    max_weight: usize,
}

impl<T> Memo<T> {
    pub fn new(max_weight: usize) -> Self {
        Memo { entries: lru::LruCache::unbounded(), weight: 0, max_weight }
    }

    fn get(&mut self, k: &Key) -> Option<Arc<T>> {
        self.entries.get(k).map(|(v, _)| v.clone())
    }

    fn put(&mut self, k: Key, v: Arc<T>, weight: usize) {
        if weight > self.max_weight / 4 {
            return;
        }
        if let Some((_, w)) = self.entries.put(k, (v, weight)) {
            self.weight -= w;
        }
        self.weight += weight;
        while self.weight > self.max_weight {
            match self.entries.pop_lru() {
                Some((_, (_, w))) => self.weight -= w,
                None => break,
            }
        }
    }
}

/// `f` over `items`, on up to every core, results in input order.
pub fn par_map<I: Sync, O: Send>(items: &[I], f: impl Fn(&I) -> O + Sync) -> Vec<O> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(items.len());
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|s| {
        // Each worker reports its notes into the render that started it.
        let f = &f;
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|c| s.spawn(crate::notes::propagate_fn(move || c.iter().map(f).collect::<Vec<O>>())))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("decode thread panicked")).collect()
    })
}

/// Decode every one of `inputs` (tile bytes) through `cache`: hits are
/// shared, misses are decoded in parallel with `decode` and kept, weighed
/// by `weight`. Results are in input order; failures are `Err`.
pub fn decode_all<T: Send + Sync, E: Send>(
    cache: &Mutex<Memo<T>>,
    inputs: &[&[u8]],
    decode: impl Fn(&[u8]) -> Result<T, E> + Sync,
    weight: impl Fn(&[u8], &T) -> usize,
) -> Vec<Result<Arc<T>, E>> {
    let keys: Vec<Key> = inputs.iter().map(|b| key(b)).collect();
    let mut out: Vec<Option<Result<Arc<T>, E>>> = {
        let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
        keys.iter().map(|k| c.get(k).map(Ok)).collect()
    };
    let missing: Vec<usize> = (0..inputs.len()).filter(|&i| out[i].is_none()).collect();
    let decoded = par_map(&missing, |&i| decode(inputs[i]).map(Arc::new));
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    for (i, r) in missing.into_iter().zip(decoded) {
        if let Ok(v) = &r {
            c.put(keys[i], v.clone(), weight(inputs[i], v));
        }
        out[i] = Some(r);
    }
    out.into_iter().map(|r| r.expect("every input decoded")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_once_and_keeps_order() {
        let cache = Mutex::new(Memo::<String>::new(1 << 20));
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let decode = |b: &[u8]| -> Result<String, ()> {
            calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if b == b"bad" { Err(()) } else { Ok(String::from_utf8(b.to_vec()).unwrap()) }
        };
        let inputs: Vec<&[u8]> = vec![b"a", b"b", b"bad", b"a", b"c"];
        let r = decode_all(&cache, &inputs, decode, |b, _| b.len());
        let got: Vec<Option<String>> = r.iter().map(|x| x.as_ref().ok().map(|s| s.to_string())).collect();
        assert_eq!(got, [Some("a".into()), Some("b".into()), None, Some("a".into()), Some("c".into())]);
        let first = calls.load(std::sync::atomic::Ordering::Relaxed);
        decode_all(&cache, &inputs, decode, |b, _| b.len());
        // Second pass: only the failure is decoded again.
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), first + 1);
    }

    #[test]
    fn par_map_keeps_order() {
        let v: Vec<u32> = (0..1000).collect();
        assert_eq!(par_map(&v, |x| x * 2), v.iter().map(|x| x * 2).collect::<Vec<_>>());
    }
}
