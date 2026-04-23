#!/usr/bin/env bash
#
# Build the routee-compass python wheel (via maturin) against a
# locally-built ONNX Runtime static library. Requires
# `scripts/hpc/build_ort.sh` to have succeeded first (or a pre-existing
# ORT build at $ORT_LIB_PATH).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

for tool in maturin; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: required tool '$tool' not found on PATH" >&2
    echo "hint: run inside the pixi hpc env (e.g. 'pixi run -e hpc build_hpc_wheel')" >&2
    exit 1
  fi
done

: "${ORT_LIB_PATH:?ORT_LIB_PATH not set (expected from pixi hpc env)}"

if [[ ! -d "${ORT_LIB_PATH}" ]]; then
  echo "error: ORT_LIB_PATH=${ORT_LIB_PATH} does not exist" >&2
  echo "hint: run 'pixi run -e hpc build_hpc_ort' first" >&2
  exit 1
fi

echo "==> building routee-compass python wheel (release, ort-static)"
( cd "${REPO_ROOT}" && \
  maturin build --release \
    --no-default-features \
    --features ort-static )

echo "==> python wheels: ${REPO_ROOT}/target/wheels/"
