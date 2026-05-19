//! Criterion benchmarks for netune-api crypto + serialization round-trips.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use netune_core::models::{Album, Artist, QualityLevel, Song};

fn make_typical_request_payload() -> serde_json::Value {
    serde_json::json!({
        "s": "周杰伦",
        "type": 1,
        "offset": 0,
        "limit": 30,
        "total": true
    })
}

fn make_song() -> Song {
    Song {
        id: 123456,
        name: "晴天".to_string(),
        artists: vec![
            Artist { id: 1, name: "周杰伦".to_string() },
        ],
        album: Album {
            id: 100,
            name: "叶惠美".to_string(),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
        },
        duration: 269_000,
        quality: QualityLevel::ExHigh,
    }
}

fn bench_weapi_request_roundtrip(c: &mut Criterion) {
    let payload = make_typical_request_payload();

    c.bench_function("full_weapi_request_encrypt", |b| {
        b.iter(|| {
            let encrypted = netune_api::crypto::weapi_encrypt(black_box(&payload)).unwrap();
            // Simulate serializing the encrypted result into a form body
            let _body = serde_json::to_string(&encrypted).unwrap();
        })
    });
}

fn bench_eapi_request_encrypt_serialize(c: &mut Criterion) {
    let payload = make_typical_request_payload();
    let json_str = payload.to_string();
    let path = "/api/cloudsearch/get/web";

    c.bench_function("full_eapi_request_encrypt_serialize", |b| {
        b.iter(|| {
            let encrypted = netune_api::crypto::encrypt_eapi(black_box(&json_str), black_box(path)).unwrap();
            let _body = serde_json::to_string(&encrypted).unwrap();
        })
    });
}

fn bench_song_deserialize_from_json(c: &mut Criterion) {
    let song = make_song();
    let json = serde_json::to_string(&song).unwrap();

    c.bench_function("song_single_deserialize", |b| {
        b.iter(|| {
            let _song: Song = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_search_response_deserialize(c: &mut Criterion) {
    // Simulate a typical search response with 30 songs
    let songs: Vec<Song> = (0..30).map(|i| Song {
        id: i,
        name: format!("Song {i}"),
        artists: vec![Artist { id: i / 10, name: format!("Artist {}", i / 10) }],
        album: Album {
            id: i / 5,
            name: format!("Album {}", i / 5),
            cover_url: Some(format!("https://example.com/cover/{i}.jpg")),
        },
        duration: 180_000 + i * 1000,
        quality: QualityLevel::ExHigh,
    }).collect();
    let json = serde_json::to_string(&songs).unwrap();

    c.bench_function("search_response_30_songs_deserialize", |b| {
        b.iter(|| {
            let _songs: Vec<Song> = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

criterion_group!(
    network_benches,
    bench_weapi_request_roundtrip,
    bench_eapi_request_encrypt_serialize,
    bench_song_deserialize_from_json,
    bench_search_response_deserialize,
);
criterion_main!(network_benches);
