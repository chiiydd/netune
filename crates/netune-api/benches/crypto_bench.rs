//! Criterion benchmarks for netune-api crypto operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn make_1kb_json() -> String {
    // Generate a ~1KB JSON string by including many IDs
    let ids: Vec<String> = (0..200).map(|i| (i * 1000).to_string()).collect();
    let data = format!(
        r#"{{"ids":"[{}]","br":320000,"csrf_token":"","padding":"{}"}}"#,
        ids.join(","),
        "x".repeat(200)
    );
    debug_assert!(data.len() >= 1000, "payload is {} bytes", data.len());
    data
}

fn bench_encrypt_linuxapi(c: &mut Criterion) {
    let data = make_1kb_json();

    c.bench_function("encrypt_linuxapi_1kb", |b| {
        b.iter(|| {
            netune_api::crypto::encrypt_linuxapi(black_box(&data)).unwrap();
        })
    });
}

fn bench_encrypt_eapi(c: &mut Criterion) {
    let data = make_1kb_json();
    let path = "/api/song/enhance/player/url";

    c.bench_function("encrypt_eapi_1kb", |b| {
        b.iter(|| {
            netune_api::crypto::encrypt_eapi(black_box(&data), black_box(path)).unwrap();
        })
    });
}

fn bench_encrypt_weapi(c: &mut Criterion) {
    let data = serde_json::json!({
        "type": 1,
        "noCheckToken": true,
        "csrf_token": ""
    });

    c.bench_function("encrypt_weapi", |b| {
        b.iter(|| {
            netune_api::crypto::weapi_encrypt(black_box(&data)).unwrap();
        })
    });
}

fn bench_aes_roundtrip(c: &mut Criterion) {
    // 10KB payload
    let payload = vec![0xAB_u8; 10 * 1024];
    let key = *b"0CoJUm6Qyw8W8jud";

    c.bench_function("aes_ecb_roundtrip_10kb", |b| {
        b.iter(|| {
            let enc = netune_api::crypto::aes_ecb_encrypt(black_box(&payload), &key).unwrap();
            let _dec = netune_api::crypto::aes_ecb_decrypt(black_box(&enc), &key).unwrap();
        })
    });
}

criterion_group!(
    crypto_benches,
    bench_encrypt_linuxapi,
    bench_encrypt_eapi,
    bench_encrypt_weapi,
    bench_aes_roundtrip,
);
criterion_main!(crypto_benches);
