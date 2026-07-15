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

# ORT pins eigen in cmake/deps.txt as a gitlab.com archive download verified by
# a SHA1 of the archive bytes. GitLab regenerates those archives over time (same
# tree, different bytes), so the pinned hash rots and fresh builds fail the
# download. Instead of chasing hashes, fetch the pinned eigen commit over git --
# commits are content-addressed and can't drift -- and hand the checkout to
# cmake via FETCHCONTENT_SOURCE_DIR_EIGEN, which makes FetchContent use the
# local source dir and skip the archive download (and its hash check) entirely.
EIGEN_COMMIT="$(sed -n 's|^eigen;https://gitlab\.com/libeigen/eigen/-/archive/\([0-9a-f]\{40\}\)/.*|\1|p' \
  "${ONNXRUNTIME_DIR}/cmake/deps.txt")"
if [[ -z "${EIGEN_COMMIT}" ]]; then
  echo "error: could not parse the eigen commit from ${ONNXRUNTIME_DIR}/cmake/deps.txt" >&2
  echo "hint: the eigen line's format changed (ORT tag bump?); update the eigen" >&2
  echo "      pre-clone logic in scripts/hpc/build_ort.sh to match." >&2
  exit 1
fi
EIGEN_SRC_DIR="$(dirname "${ONNXRUNTIME_DIR}")/eigen-${EIGEN_COMMIT}"
if [[ "$(git -C "${EIGEN_SRC_DIR}" rev-parse HEAD 2>/dev/null)" == "${EIGEN_COMMIT}" ]]; then
  echo "==> reusing eigen checkout at ${EIGEN_SRC_DIR}"
else
  echo "==> fetching eigen ${EIGEN_COMMIT} into ${EIGEN_SRC_DIR}"
  rm -rf "${EIGEN_SRC_DIR}"
  git init -q "${EIGEN_SRC_DIR}"
  git -C "${EIGEN_SRC_DIR}" remote add origin https://gitlab.com/libeigen/eigen.git
  git -C "${EIGEN_SRC_DIR}" fetch -q --depth 1 origin "${EIGEN_COMMIT}"
  git -C "${EIGEN_SRC_DIR}" checkout -q --detach FETCH_HEAD
fi

echo "==> building onnxruntime (${ORT_BUILD_CONFIG})"
( cd "${ONNXRUNTIME_DIR}" && \
  ./build.sh \
    --config "${ORT_BUILD_CONFIG}" \
    --parallel \
    --compile_no_warning_as_error \
    --skip_submodule_sync \
    --skip_tests \
    --cmake_extra_defines "FETCHCONTENT_SOURCE_DIR_EIGEN=${EIGEN_SRC_DIR}" )

echo "==> ORT static libs at ${ORT_LIB_DIR}"
