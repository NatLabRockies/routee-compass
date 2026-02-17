#!/bin/bash

# Generate schema to temp file
cargo run --manifest-path rust/Cargo.toml --bin compass-schema > /tmp/compass-schema.json

# Compare with committed version
if ! diff -q docs/compass-config-schema.json /tmp/compass-schema.json > /dev/null; then
  echo "Schema is out of date. Run ./scripts/generate-compass-schema.sh"
  exit 1
fi

cp /tmp/compass-schema.json docs/compass-config-schema.json
