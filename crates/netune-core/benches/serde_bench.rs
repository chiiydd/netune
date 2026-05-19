//! Criterion benchmarks for netune-core JSON serialization.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use netune_core::config::Config;
use netune_core::models::{Album, Artist, QualityLevel, SearchResult, Song};

/// Create a test song with a given id.
fn make_song(id: u64) -> Song {
    Song {
        id,
        name: format!("Benchmark Song {id}"),
        artists: vec![Artist {
            id: id / 10,
            name: format!("Artist {}", id / 10),
        }],
        album: Album {
            id: id / 5,
            name: format!("Album {}", id / 5),
            cover_url: Some(format!("https://example.com/cover/{id}.jpg")),
        },
        duration: 180_000 + id * 1000,
        quality: QualityLevel::ExHigh,
    }
}

/// Create 1000 songs for benchmarking.
fn make_songs_1000() -> Vec<Song> {
    (0..1000).map(make_song).collect()
}

fn bench_song_serialize(c: &mut Criterion) {
    let songs = make_songs_1000();

    c.bench_function("song_serialize_1000", |b| {
        b.iter(|| {
            let _json = serde_json::to_string(black_box(&songs)).unwrap();
        })
    });
}

fn bench_song_deserialize(c: &mut Criterion) {
    let songs = make_songs_1000();
    let json = serde_json::to_string(&songs).unwrap();

    c.bench_function("song_deserialize_1000", |b| {
        b.iter(|| {
            let _songs: Vec<Song> = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_search_result_serialize(c: &mut Criterion) {
    let result = SearchResult {
        songs: (0..100).map(make_song).collect(),
        total: 1000,
        has_more: true,
    };

    c.bench_function("search_result_serialize_100", |b| {
        b.iter(|| {
            let _json = serde_json::to_string(black_box(&result)).unwrap();
        })
    });
}

fn bench_config_roundtrip(c: &mut Criterion) {
    let config = Config {
        quality: QualityLevel::Lossless,
        volume: 0.75,
        show_translation: true,
    };

    c.bench_function("config_serialize", |b| {
        b.iter(|| {
            let _json = serde_json::to_string(black_box(&config)).unwrap();
        })
    });

    let json = serde_json::to_string(&config).unwrap();
    c.bench_function("config_deserialize", |b| {
        b.iter(|| {
            let _config: Config = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

criterion_group!(
    serde_benches,
    bench_song_serialize,
    bench_song_deserialize,
    bench_search_result_serialize,
    bench_config_roundtrip,
);
criterion_main!(serde_benches);
