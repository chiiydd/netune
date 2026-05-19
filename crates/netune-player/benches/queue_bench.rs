//! Criterion benchmarks for netune-player queue operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use netune_core::models::{Album, Artist, QualityLevel, Song};
use netune_player::queue::{PlayMode, PlayQueue};

/// Create a test song with a given id and name.
fn make_song(id: u64, name: &str) -> Song {
    Song {
        id,
        name: name.to_string(),
        artists: vec![Artist {
            id: 1,
            name: "Benchmark Artist".to_string(),
        }],
        album: Album {
            id: 1,
            name: "Benchmark Album".to_string(),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
        },
        duration: 180_000,
        quality: QualityLevel::ExHigh,
    }
}

/// Create a queue pre-loaded with N songs.
fn queue_with_n(n: usize) -> PlayQueue {
    let mut q = PlayQueue::new();
    for i in 0..n {
        q.push(make_song(i as u64, &format!("Song {i}")));
    }
    q
}

fn bench_queue_push_1000(c: &mut Criterion) {
    c.bench_function("queue_push_1000", |b| {
        b.iter(|| {
            let mut q = PlayQueue::new();
            for i in 0..1000 {
                q.push(black_box(make_song(i, &format!("Song {i}"))));
            }
            q
        })
    });
}

fn bench_queue_advance_sequential(c: &mut Criterion) {
    c.bench_function("queue_advance_sequential_1000", |b| {
        b.iter_batched(
            || {
                let mut q = queue_with_n(1000);
                q.set_repeat_mode(PlayMode::Sequential);
                q
            },
            |mut q| {
                while q.advance().is_some() {}
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_queue_shuffle_1000(c: &mut Criterion) {
    c.bench_function("queue_shuffle_1000", |b| {
        b.iter_batched(
            || queue_with_n(1000),
            |mut q| {
                q.shuffle();
                q
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_queue_iterator_100(c: &mut Criterion) {
    c.bench_function("queue_iterator_100_sequential", |b| {
        b.iter_batched(
            || {
                let mut q = queue_with_n(100);
                q.set_repeat_mode(PlayMode::Sequential);
                q
            },
            |mut q| {
                let _songs: Vec<_> = q.by_ref().map(|s| s.name.clone()).collect();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_queue_save_load(c: &mut Criterion) {
    let songs: Vec<Song> = (0..100)
        .map(|i| make_song(i, &format!("Song {i}")))
        .collect();
    let mut q = PlayQueue::new();
    q.load(songs);
    q.set_repeat_mode(PlayMode::LoopAll);
    q.advance();
    q.advance();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queue_bench.json");

    c.bench_function("queue_save_100_songs", |b| {
        b.iter(|| {
            q.save_to_file(black_box(&path)).unwrap();
        })
    });

    c.bench_function("queue_load_100_songs", |b| {
        b.iter_batched(
            || {
                q.save_to_file(&path).unwrap();
            },
            |_| {
                let _loaded = PlayQueue::load_from_file(black_box(&path)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    queue_benches,
    bench_queue_push_1000,
    bench_queue_advance_sequential,
    bench_queue_shuffle_1000,
    bench_queue_iterator_100,
    bench_queue_save_load,
);
criterion_main!(queue_benches);
