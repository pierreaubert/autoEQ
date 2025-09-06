#!/bin/bash

cargo clean
rm -fr dist
rm -fr data_generated/*
cd ui && rm -fr node_modules && rm -fr gen/schemas/* && cd ..
cd ui/src-tauri && cargo clean && cd ../..
find . -name '*~' -exec rm {} \; -print
find . -name 'Cargo.lock' -exec rm {} \; -print
find . -name 'package-lock.json' -exec rm {} \; -print
