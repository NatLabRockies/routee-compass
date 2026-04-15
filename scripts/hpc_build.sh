#!/usr/bin/env bash
#
# Build routee-compass on an HPC where prebuilt ONNX Runtime binaries
# can't be used (e.g. host glibc is too old for crates.io's `ort`
# `download-binaries` feature). Clones onnxruntime, builds it from source
# as a static library, then builds the routee-compass rust workspace and
# python wheel against it.
#
# Configurable via environment variables:
#   ONNXRUNTIME_DIR    where to clone/build ORT (default: <repo>/build/onnxruntime)
#   ONNXRUNTIME_TAG    git tag/branch to check out (default: v1.20.1)
#   ORT_BUILD_CONFIG   ORT build config (default: RelWithDebInfo)
#   SKIP_ORT_BUILD     if set, skip the ORT clone+build step (assumes libs already exist)
#
# Recommended: run inside the project's pixi environment so cmake, cargo
# and maturin are all on PATH:
#   pixi run bash scripts/hpc_build.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ONNXRUNTIME_DIR="${ONNXRUNTIME_DIR:-${REPO_ROOT}/build/onnxruntime}"
ONNXRUNTIME_TAG="${ONNXRUNTIME_TAG:-v1.20.1}"
ORT_BUILD_CONFIG="${ORT_BUILD_CONFIG:-RelWithDebInfo}"

for tool in cmake cargo maturin git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: required tool '$tool' not found on PATH" >&2
    echo "hint: run inside the pixi env (e.g. 'pixi run bash $0')" >&2
    exit 1
  fi
done

if [[ ! -d "${ONNXRUNTIME_DIR}/.git" ]]; then
  echo "==> cloning onnxruntime ${ONNXRUNTIME_TAG} into ${ONNXRUNTIME_DIR}"
  mkdir -p "$(dirname "${ONNXRUNTIME_DIR}")"
  git clone --depth 1 --branch "${ONNXRUNTIME_TAG}" --recurse-submodules \
    https://github.com/microsoft/onnxruntime.git "${ONNXRUNTIME_DIR}"
else
  echo "==> reusing existing onnxruntime checkout at ${ONNXRUNTIME_DIR}"
fi

ORT_LIB_DIR="${ONNXRUNTIME_DIR}/build/Linux/${ORT_BUILD_CONFIG}"

if [[ -n "${SKIP_ORT_BUILD:-}" ]]; then
  echo "==> SKIP_ORT_BUILD set, skipping ORT build"
  if [[ ! -d "${ORT_LIB_DIR}" ]]; then
    echo "error: ${ORT_LIB_DIR} does not exist; cannot skip build" >&2
    exit 1
  fi
else
  echo "==> building onnxruntime (${ORT_BUILD_CONFIG})"
  ( cd "${ONNXRUNTIME_DIR}" && \
    ./build.sh \
      --config "${ORT_BUILD_CONFIG}" \
      --parallel \
      --compile_no_warning_as_error \
      --skip_submodule_sync \
      --skip_tests )
fi

export ORT_LIB_PATH="${ORT_LIB_DIR}"
echo "==> ORT_LIB_PATH=${ORT_LIB_PATH}"

echo "==> building routee-compass rust workspace (release)"
( cd "${REPO_ROOT}/rust" && \
  cargo build --workspace --release \
    --no-default-features \
    --features routee-compass-powertrain/ort-static )

echo "==> building routee-compass python wheel"
( cd "${REPO_ROOT}" && \
  maturin build --release \
    --no-default-features \
    --features routee-compass-powertrain/ort-static )

echo "==> done"
echo "  rust binaries: ${REPO_ROOT}/rust/target/release/"
echo "  python wheels: ${REPO_ROOT}/target/wheels/"
