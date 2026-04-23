#!/usr/bin/env bash
#
# Clone and build ONNX Runtime as a static library for HPC systems where
# prebuilt ORT binaries can't be used (e.g. host glibc is too old for
# crates.io's `ort` `download-binaries` feature).
#
# Configurable via environment variables (exported by the pixi `hpc`
# feature activation when run via `pixi run`):
#   ONNXRUNTIME_DIR    where to clone/build ORT
#   ONNXRUNTIME_TAG    git tag/branch to check out
#   ORT_BUILD_CONFIG   ORT build config (e.g. RelWithDebInfo, Release)
#   SKIP_ORT_BUILD     if set, skip the ORT build step (reuse existing libs)

set -euo pipefail

for tool in cmake git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: required tool '$tool' not found on PATH" >&2
    echo "hint: run inside the pixi hpc env (e.g. 'pixi run -e hpc build_hpc_ort')" >&2
    exit 1
  fi
done

: "${ONNXRUNTIME_DIR:?ONNXRUNTIME_DIR not set (expected from pixi hpc env)}"
: "${ONNXRUNTIME_TAG:?ONNXRUNTIME_TAG not set (expected from pixi hpc env)}"
: "${ORT_BUILD_CONFIG:?ORT_BUILD_CONFIG not set (expected from pixi hpc env)}"

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
  exit 0
fi

echo "==> building onnxruntime (${ORT_BUILD_CONFIG})"
( cd "${ONNXRUNTIME_DIR}" && \
  ./build.sh \
    --config "${ORT_BUILD_CONFIG}" \
    --parallel \
    --compile_no_warning_as_error \
    --skip_submodule_sync \
    --skip_tests )

echo "==> ORT static libs at ${ORT_LIB_DIR}"
