#!/bin/bash

cargo run --manifest-path rust/Cargo.toml --bin compass-schema > docs/compass-config-schema.json
