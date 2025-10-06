<!-- markdownlint-disable-file MD013 -->

# AutoEQ : an automatic eq for your speaker or headset

The software help you find very good EQ for your speaker or your headset. It is available as a CLI or as an application.

## Install

### Cargo

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup:

```shell
cargo install autoeq
```

and you are set up. See this [README](src-autoeq/README.md) for instructions on how to use it.

## Toolkit

### src-env

A small set of functions and constants used by the other crates but you are unlikely to be interested.

### src-testfunctions

A [set of functions](src-testfunctions/README.md) for testing non linear optimisation algorithms

### src-de

A implementation of [differential evolution algorithm](src-de/README.md) (forked from Scipy) with an interface to NLopt and MetaHeuristics two libraries that also provide various optimisation algorithms. DE support linear and non-linear constraints and implement other features like JADE or adaptative behaviour.

### src-cea2034

A implementation of CEA2034 aka [Spinorama](https://spinorama.org): a set of metrics and curves that describe a loudspeaker performance.

### src-autoeq

A [CLI](src-autoeq/README.md) to optimise the response of your headset or headphone.
A corresponing App is also available at [https://github.com/pierreaubert/autoeq-app](https://github.com/pierreaubert/autoeq-app).
