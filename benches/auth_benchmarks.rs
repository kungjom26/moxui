//! Benchmarks for auth-critical paths: JWT encode/decode, password hashing.
//!
//! Run with: `cargo bench`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_jwt_encode(c: &mut Criterion) {
    let priv_pem = include_bytes!("../tests/fixtures/test_jwt_priv.pem");
    let pub_pem = include_bytes!("../tests/fixtures/test_jwt_pub.pem");
    let jwt = moxui::auth::JwtService::new(priv_pem, pub_pem, "bench", "bench").unwrap();

    let claims = moxui::auth::Claims {
        sub: "u-bench".to_string(),
        username: "benchmarker".to_string(),
        role: "admin".to_string(),
        iat: 1_700_000_000,
        exp: 1_700_003_600,
    };

    c.bench_function("jwt_encode", |b| {
        b.iter(|| {
            let token = jwt.encode(black_box(&claims)).unwrap();
            black_box(token)
        })
    });
}

fn bench_jwt_decode(c: &mut Criterion) {
    let priv_pem = include_bytes!("../tests/fixtures/test_jwt_priv.pem");
    let pub_pem = include_bytes!("../tests/fixtures/test_jwt_pub.pem");
    let jwt = moxui::auth::JwtService::new(priv_pem, pub_pem, "bench", "bench").unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = moxui::auth::Claims {
        sub: "u-bench".to_string(),
        username: "benchmarker".to_string(),
        role: "admin".to_string(),
        iat: now,
        exp: now + 3600,
    };
    let token = jwt.encode(&claims).unwrap();

    c.bench_function("jwt_decode", |b| {
        b.iter(|| {
            let decoded = jwt.decode(black_box(&token)).unwrap();
            black_box(decoded)
        })
    });
}

fn bench_password_hash(c: &mut Criterion) {
    c.bench_function("bcrypt_hash", |b| {
        b.iter(|| {
            let hash = moxui::auth::password::hash_password(black_box("hunter2-bench-pwd")).unwrap();
            black_box(hash)
        })
    });
}

fn bench_password_verify(c: &mut Criterion) {
    let hash = moxui::auth::password::hash_password("hunter2-bench-pwd").unwrap();

    c.bench_function("bcrypt_verify", |b| {
        b.iter(|| {
            let ok = moxui::auth::password::verify_password(
                black_box("hunter2-bench-pwd"),
                black_box(&hash),
            )
            .unwrap();
            black_box(ok)
        })
    });
}

criterion_group!(benches, bench_jwt_encode, bench_jwt_decode, bench_password_hash, bench_password_verify);
criterion_main!(benches);