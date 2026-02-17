#!/bin/bash

# Create temporary file
TEMP_SCHEMA=$(mktemp)
trap "rm -f $TEMP_SCHEMA" EXIT

# Generate schema to temp file
cargo run --manifest-path rust/Cargo.toml --bin compass-schema > "$TEMP_SCHEMA"

# Compare with committed version
if ! diff -q docs/compass-config-schema.json "$TEMP_SCHEMA" > /dev/null; then
  echo "Schema is out of date. Run ./scripts/generate-compass-schema.sh"
  exit 1
fi
