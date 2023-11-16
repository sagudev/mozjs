set export

RUSTFLAGS := "-Zsanitizer=leak"
#CFLAGS := "-Zsanitizer=leak"
#CXXFLAGS := "-Zsanitizer=leak"

test:
    cargo +nightly test

minimal:
    cargo +nightly run --example minimal