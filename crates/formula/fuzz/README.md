# Formula fuzz target

Run with `cargo +nightly fuzz run parser` after installing `cargo-fuzz`. The target exercises tokenization and parsing on arbitrary UTF-8 input and is intentionally kept outside the main Cargo workspace.
