default:
    just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test: rust-test ts-test

rust-test:
	cargo check
	cargo test --lib

ts-test:
	cd src-ui && npm run test

# ----------------------------------------------------------------------
# BUILD
# ----------------------------------------------------------------------

bin: rust-bin ts-bin

rust-bin:
	cargo build --release --workspace
	cargo build --release --bin autoeq
	cargo build --release --bin plot_functions

ts-bin:
	cd src-ui && npm run tauri build

dev: rust-dev ts-dev

rust-dev:
	cargo build --workspace

ts-dev:
	cd src-ui && npm run tauri dev

download: rust-bin
	cargo run --bin download

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: rust-update ts-update pre-commit-update

rust-update:
	rustup update
	cargo update

ts-update:
	cd src-ui && npm run tauri update && npm run upgrade

pre-commit-update:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: rust-demo ts-demo

rust-demo: headphone_loss_demo print_functions

headphone_loss_demo:
	cargo run --example headphone_loss_demo -- --spl "./data_tests/headphone/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" --target "./data_tests/targets/harman-over-ear-2018.csv"

print_functions:
	cargo run --bin print_functions

ts-demo:
	cd src-ui && npm run build:audio-player
