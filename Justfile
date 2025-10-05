# ----------------------------------------------------------------------
# How to install Just?
#     cargo install just
# ----------------------------------------------------------------------

default:
    just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test: test-rust test-ts

test-rust:
	cargo check --all-targets
	cargo test --lib

test-ts:
	cd src-ui && npm run test

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

prod: prod-rust prod-ts

prod-rust:
	cargo build --release --workspace
	cargo build --release --bin autoeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_autoeq_speaker
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

prod-ts:
	cd src-ui && npm run tauri build

# ----------------------------------------------------------------------
# BENCH
# ----------------------------------------------------------------------

bench: bench-convergence bench-autoeq-speaker

bench-convergence:
	cargo run --release --bin benchmark_convergence

bench-autoeq-speaker:
	cargo run --release --bin benchmark_autoeq_speaker

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

dev: dev-rust dev-ts

dev-rust:
	cargo build --workspace
	cargo build --bin autoeq
	cargo build --bin plot_functions
	cargo build --bin download
	cargo build --bin benchmark_convergence
	cargo build --bin benchmark_autoeq_speaker
	cargo build --bin plot_autoeq_de
	cargo build --bin run_autoeq_de

dev-ts:
	cd src-ui && npm run tauri dev

download:
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

demo-rust: headphone_loss_demo plot_functions

headphone_loss_demo:
	cargo run --example headphone_loss_demo -- --spl "./data_tests/headphone/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" --target "./data_tests/targets/harman-over-ear-2018.csv"

plot_functions:
	cargo run --bin plot_functions

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

# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

cross : cross-linux-x86

cross-linux-x86 :
      echo "This can take minutes!"
      cd src-ui/src-tauri && cross build --release --target x86_64-unknown-linux-gnu

cross-win-x86-gnu :
      echo "This is not working well yet from macos!"
      cd src-ui/src-tauri && cross build --release --target x86_64-pc-windows-gnu

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

install-cross:
	rustup target add x86_64-apple-ios

install-macos:
	# need rustup first
	# need xcode
	xcode-select --install
	# need brew
	brew install chromedriver
	xattr -d com.apple.quarantine $(which chromedriver)
	brew install npm
	# optimisation
	brew install nlopt cmake

install-macos: install-ios
	# more stuff
	rustup target add aarch64-apple-ios           # For physical iPad devices
	rustup target add aarch64-apple-ios-sim       # For ARM64 iPad simulator
	# app UI
	brew install cocoapods
	# need npm
	npm run tauri ios init
