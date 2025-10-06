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

prod: prod-workspace prod-autoeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_autoeq_speaker
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

prod-workspace:
	cargo build --release --workspace

prod-autoeq:
	cargo build --release --bin autoeq

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

install-macos-doc:
	brew install chruby ruby-install
	gem install jekyll

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

install-linux-arm:
	sudo apt install \
	     gcc g++ cmake \
	     libnlopt-dev \
	     rustup \
	     just \
	     libopenblas64-dev libopenblas64-0-pthread \
	     libssl-dev \
	     pkg-config \
	     ca-certificates \
	     chromium-chromedriver
	rustup default stable
	rustup target add aarch64-unknown-linux-gnu

# ----------------------------------------------------------------------
# publish
# ----------------------------------------------------------------------

publish:
        cargo publish

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

qa: prod-autoeq qa-ascilab-6b qa-jbl-m2-flat qa-jbl-m2-score

qa-ascilab-6b:
        ./target/release/autoeq --speaker="Ascilab F6B" --version asr --measurement CEA2034 --algo autoeq:de --loss speaker-score -n 7 --min-freq=30 --max-q=6 --qa | ./scripts/qa_check.sh

qa-jbl-m2-flat:
        ./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 --algo autoeq:de --loss speaker-flat -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk --qa | ./scripts/qa_check.sh

qa-jbl-m2-score:
        ./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 --algo autoeq:de --loss speaker-score -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk --qa | ./scripts/qa_check.sh

qa-beyerdynamic-dt1990pro:
	./target/release/autoeq -n 4 --curve ./data_tests/headphone/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv --target ./data_tests/targets/harman-over-ear-2018.csv --loss headphone-score  --qa | ./scripts/qa_check.sh
