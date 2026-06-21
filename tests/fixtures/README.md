# Test-only RSA keypair for unit tests in `src/auth/jwt.rs` and
# `src/auth/middleware.rs`.
#
# Generated via `openssl genrsa 2048` and committed ONLY because unit
# tests need a deterministic PEM (compile-time `include_str!`).
# This key MUST NEVER be used in production. `main.rs` does not
# reference these paths — production keys come from
# `MOXUI_AUTH__JWT_PRIVATE_KEY_PEM_PATH` (see config.yaml).
#
# Regenerate locally with:
#   openssl genrsa -out test_jwt_priv.pem 2048
#   openssl rsa -in test_jwt_priv.pem -pubout -out test_jwt_pub.pem
