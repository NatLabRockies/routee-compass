#!/usr/bin/env bash
#
# Back-compat wrapper: run the full HPC build end-to-end (ORT + rust +
# wheel). Prefer `pixi run -e hpc build_hpc` for new usage, which picks
# up the same env vars via pixi's `hpc` feature activation.
#
# For individual phases, see scripts/hpc/{build_ort,build_rust,build_wheel}.sh
# or `pixi run -e hpc build_hpc_{ort,rust,wheel}`.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Defaults (when not running under `pixi run -e hpc`, which already exports these).
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export ONNXRUNTIME_DIR="${ONNXRUNTIME_DIR:-${REPO_ROOT}/build/onnxruntime}"
export ONNXRUNTIME_TAG="${ONNXRUNTIME_TAG:-v1.20.1}"
export ORT_BUILD_CONFIG="${ORT_BUILD_CONFIG:-RelWithDebInfo}"
export ORT_LIB_PATH="${ORT_LIB_PATH:-${ONNXRUNTIME_DIR}/build/Linux/${ORT_BUILD_CONFIG}}"

bash "${SCRIPT_DIR}/hpc/build_ort.sh"
bash "${SCRIPT_DIR}/hpc/build_rust.sh"
bash "${SCRIPT_DIR}/hpc/build_wheel.sh"

echo "==> done"
echo "  rust binaries: ${REPO_ROOT}/rust/target/release/"
echo "  python wheels: ${REPO_ROOT}/target/wheels/"
