default:
    just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test: test-rust test-ts

test-rust:
	cargo check
	cargo test --lib

test-ts:
	cd src-ui && npm run test

# ----------------------------------------------------------------------
# BUILD
# ----------------------------------------------------------------------

bin: bin-rust bin-ts

bin-rust:
	cargo build --release --workspace
	cargo build --release --bin autoeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

bin-ts:
	cd src-ui && npm run tauri build

dev: dev-rust dev-ts

dev-rust:
	cargo build --workspace

dev-ts:
	cd src-ui && npm run tauri dev

download: bin-rust
	cargo run --bin download

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: update-rust update-ts update-pre-commit

update-rust:
	rustup update
	cargo update

update-ts:
	cd src-ui && npm run tauri update && npm run upgrade

update-pre-commit:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: demo-rust demo-ts

demo-rust: headphone_loss_demo print_functions

headphone_loss_demo:
	cargo run --example headphone_loss_demo -- --spl "./data_tests/headphone/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" --target "./data_tests/targets/harman-over-ear-2018.csv"

print_functions:
	cargo run --bin print_functions

demo-ts:
	cd src-ui && npm run build:audio-player

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

examples : examples-iir examples-de

examples-iir :
        cargo run --example format_demo
        cargo run --example readme_example

examples-de :
        cargo run --example optde_basic
        cargo run --example optde_adaptive_demo
        cargo run --example optde_linear_constraints
        cargo run --example optde_nonlinear_constraints
        cargo run --example optde_parallel
