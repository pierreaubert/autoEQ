#!/bin/bash

cargo clean
rm -fr dist
rm -fr data_generated/*
find . -name '*~' -exec rm {} \; -print
find . -name 'Cargo.lock' -exec rm {} \; -print
