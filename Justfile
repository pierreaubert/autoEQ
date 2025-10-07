# --------------------------------------------------------- -*- just -*-
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
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt: fmt-rust

fmt-rust:
    cargo fmt --all

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

alias build := prod

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
    # either jobs=1 or --no-parallel ; or a mix if you have a lot of
    # CPU cores
    cargo run --release --bin benchmark_autoeq_speaker -- --qa --jobs 1

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
    cargo run --release --example headphone_loss_demo -- \
        --spl "./data_tests/headphone/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" \
        --target "./data_tests/targets/harman-over-ear-2018.csv"

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

install-brew:
    curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh > install-brew
    chmod +x install-brew
    NONINTERACTIVE=1 ./install-brew

install-rustup:
    curl https://sh.rustup.rs -sSf > install-rustup
    chmod +x install-rustup
    ./install-rustup -y
    source ~/.cargo/env

install-macos: install-brew install-rustup
    # need rustup first
    # need xcode
    xcode-select --install

    brew install chromedriver
    xattr -d com.apple.quarantine $(which chromedriver)
    # optimisation
    brew install nlopt cmake

install-macos-doc:
    brew install chruby ruby-install
    ruby-install ruby 3.4.6
    # if above line does not work out of the box, then
    # cd ~/src/ruby-3.4.6 && make install
    PATH=$HOME/.rubies/ruby-3.4.6/bin:$PATH gem install jekyll

# ----------------------------------------------------------------------
# Install linux
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
# Install windows
# ----------------------------------------------------------------------

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

install-windows-vcpkg:
    git clone https://github.com/microsoft/vcpkg.git
    cd vcpkg; .\bootstrap-vcpkg.bat
    vcpkg install nlopt openblas

install-windows-node:
    echo "go to https://nodejs.org/en/download"

install-windows-llvm-arm:
    echo "go to https://learn.arm.com/install-guides/llvm-woa/"

install-windows-arm: install-windows-vcpkg install-windows-node install-windows-llvm-arm
    rustup deafult stable
    rustup target add aarch64-pc-windows-msvc

install-windows-x86: install-windows-vcpkg install-windows-node
    rustup default stable
    rustup target add x86_64-pc-windows-msvc

# ----------------------------------------------------------------------
# publish
# ----------------------------------------------------------------------

publish:
    cd src-testfunctions && cargo publish
    cd src-de && cargo publish
    cd src-cea2034 && cargo publish
    cd src-autoeq && cargo publish

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

qa: prod-autoeq \
    qa-ascilab-6b \
    qa-jbl-m2-flat qa-jbl-m2-score \
    qa-beyerdynamic-dt1990pro-flat qa-beyerdynamic-dt1990pro-score  qa-beyerdynamic-dt1990pro-score2 \
    qa-edifierw830nb

qa-ascilab-6b:
        ./target/release/autoeq --speaker="Ascilab F6B" --version asr --measurement CEA2034 \
            --algo autoeq:de --loss speaker-score -n 7 --min-freq=30 --max-q=6 --qa \
            | ./scripts/qa_check.sh

qa-jbl-m2-flat:
        ./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
            --algo autoeq:de --loss speaker-flat -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk \
            --qa \
            | ./scripts/qa_check.sh

qa-jbl-m2-score:
        ./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
        --algo autoeq:de --loss speaker-score -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk --qa \
        | ./scripts/qa_check.sh

qa-beyerdynamic-dt1990pro-score:
    ./target/release/autoeq -n 4 \
        --curve ./data_tests/headphone/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
        --target ./data_tests/targets/harman-over-ear-2018.csv --loss headphone-score  \
        --qa \
        | ./scripts/qa_check.sh

qa-beyerdynamic-dt1990pro-score2:
    ./target/release/autoeq -n 5 \
        --curve ./data_tests/headphone/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
        --target ./data_tests/targets/harman-over-ear-2018.csv \
        --loss headphone-score  --max-db 6 --max-q 6 --algo mh:rga --maxeval 20000 --min-freq=20 --max-freq 10000 --peq-model hp-pk-lp \
        --qa \
        | ./scripts/qa_check.sh

qa-beyerdynamic-dt1990pro-flat:
    ./target/release/autoeq -n 5 \
        --curve ./data_tests/headphone/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
        --target ./data_tests/targets/harman-over-ear-2018.csv \
        --loss headphone-flat  --max-db 6 --max-q 6 --maxeval 20000 --algo mh:pso --min-freq=20 --max-freq 10000 --peq-model pk \
        --qa \
        | ./scripts/qa_check.sh

qa-edifierw830nb:
    ./target/release/autoeq -n 5 \
        --curve data_tests/headphone/asr/edifierw830nb/Edifier\ W830NB.csv \
        --target ./data_tests/targets/harman-over-ear-2018.csv \
        --min-freq 50 --max-freq 16000 --max-q 6 --max-db 6 \
        --loss headphone-score --smooth --smooth-n 1 --peq-model pk \
        --qa \
        | ./scripts/qa_check.sh
