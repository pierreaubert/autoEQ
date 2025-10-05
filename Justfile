# ----------------------------------------------------------------------
# How to install Just?
#     cargo install just
# ----------------------------------------------------------------------

default:
    just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test:
	cargo check --all-targets
	cargo test --lib

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

prod:
	cargo build --release --workspace
	cargo build --release --bin autoeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_autoeq_speaker
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

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

dev:
	cargo build --workspace
	cargo build --bin autoeq
	cargo build --bin plot_functions
	cargo build --bin download
	cargo build --bin benchmark_convergence
	cargo build --bin benchmark_autoeq_speaker
	cargo build --bin plot_autoeq_de
	cargo build --bin run_autoeq_de

download:
	cargo run --bin download

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: update-rust update-pre-commit

update-rust:
	rustup update
	cargo update

update-pre-commit:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: headphone_loss_demo plot_functions

headphone_loss_demo:
	cargo run --release --example headphone_loss_demo -- --spl "./data_tests/headphone/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" --target "./data_tests/targets/harman-over-ear-2018.csv"

plot_functions:
	cargo run --release --bin plot_functions

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

examples : examples-iir examples-de examples-autoeq examples-testfunctions

examples-iir :
        cargo run --release --example format_demo
        cargo run --release --example readme_example

examples-de :
        cargo run --release --example optde_basic
        cargo run --release --example optde_adaptive_demo
        cargo run --release --example optde_linear_constraints
        cargo run --release --example optde_nonlinear_constraints
        cargo run --release --example optde_parallel

examples-autoeq:
	cargo run --release --example headphone_loss_validation

examples-testfunctions:
	cargo run --release --example test_hartman_4d

# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

cross : cross-linux-x86

cross-linux-x86 :
      echo "This can take minutes!"
      cross build --release --target x86_64-unknown-linux-gnu

cross-win-x86-gnu :
      echo "This is not working well yet from macos!"
      cross build --release --target x86_64-pc-windows-gnu

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

install-cross:
	rustup target add x86_64-apple-ios

install-macos:
	# need rustup first
	# need xcode
	xcode-select --install

	brew install chromedriver
	xattr -d com.apple.quarantine $(which chromedriver)
	# optimisation
	brew install nlopt cmake

# ----------------------------------------------------------------------
# publish
# ----------------------------------------------------------------------

publish:
        cargo publish

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------
qa:
        cargo run --bin autoeq --release -- --speaker="Ascilab F6B" --version asr --measurement CEA2034 --algo autoeq:de --loss speaker-score -n 7 --min-freq=30 --max-q=6
