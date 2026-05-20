//! Criterion benchmarks for DiskAudioCache put/get operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use netune_tui::audio_cache::DiskAudioCache;

const DEFAULT_MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

fn bench_cache_put_1mb(c: &mut Criterion) {
    let data = vec![0xAB_u8; 1024 * 1024];
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("cache_put_1mb", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let cache =
                    DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
                (cache, dir)
            },
            |(mut cache, _dir)| {
                rt.block_on(cache.put(1, black_box(&data)));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_cache_get_1mb(c: &mut Criterion) {
    let data = vec![0xAB_u8; 1024 * 1024];
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("cache_get_1mb", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let mut cache =
                    DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
                rt.block_on(cache.put(1, &data));
                (cache, dir)
            },
            |(cache, _dir)| {
                let _bytes = rt.block_on(cache.get(black_box(1)));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_cache_put_5mb(c: &mut Criterion) {
    let data = vec![0xCD_u8; 5 * 1024 * 1024];
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("cache_put_5mb", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let cache =
                    DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
                (cache, dir)
            },
            |(mut cache, _dir)| {
                rt.block_on(cache.put(2, black_box(&data)));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_cache_get_5mb(c: &mut Criterion) {
    let data = vec![0xCD_u8; 5 * 1024 * 1024];
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("cache_get_5mb", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let mut cache =
                    DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
                rt.block_on(cache.put(2, &data));
                (cache, dir)
            },
            |(cache, _dir)| {
                let _bytes = rt.block_on(cache.get(black_box(2)));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    cache_benches,
    bench_cache_put_1mb,
    bench_cache_get_1mb,
    bench_cache_put_5mb,
    bench_cache_get_5mb,
);
criterion_main!(cache_benches);
