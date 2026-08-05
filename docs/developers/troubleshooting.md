# Troubleshooting

This page covers some common troubleshooting issues:

  - [MacOS build error "ld: symbol(s) not found for architecture arm64"](#macos-build-error-ld-symbols-not-found-for-architecture-arm64)
  - [Building Compass without ort dependency](#building-compass-without-ort-dependency)
  - [Challenges building Compass due to ort dependency](#challenges-building-compass-due-to-ort-dependency)

## MacOS build error "ld: symbol(s) not found for architecture arm64"

If you are having installation problems on MacOS when installing the Rust dependencies for `routee-compass` via `cargo build --release`, please read through the following: 

### Problem
The MacOS linker can create issues when trying to link Python binaries with Rust's Python wrapper, PyO3. This occurs because PyO3 leaves Python symbols unresolved until runtime, but the macOS linker is incredibly strict and rejects any undefined symbols during compilation.

E.g., for `arm64`, the build error will look like the following:
```text
ld: symbol(s) not found for architecture arm64
clang: error: linker command failed with exit code 1 (use -v to see invocation)
error: could not compile `routee-compass-py` (lib) due to 1 previous error
```

### Fix

You need to tell the macOS linker to allow dynamic lookups for Python symbols at runtime. You can apply this fix globally by creating or editing your user-level Cargo configuration file:

1. Open `~/.cargo/config.toml` in your terminal:
   ```bash
   nano ~/.cargo/config.toml
   ```

2. Add the following lines to handle both Apple Silicon (arm64) and Intel (x86_64) Mac architectures:
   ```toml
   [target.x86_64-apple-darwin]
   rustflags = [ "-C", "link-arg=-undefined", "-C", "link-arg=dynamic_lookup" ]

   [target.aarch64-apple-darwin]
   rustflags = [ "-C", "link-arg=-undefined", "-C", "link-arg=dynamic_lookup" ]
   ```

3. Save the file and rerun your installation command.


## Building Compass without ort dependency

RouteE Compass has a default feature dependency on the
routee-compass-powertrain crate, which in turn depends on the [ort](https://crates.io/crates/ort) crate for running [ONNX](https://onnx.ai/)-based energy estimation models.
It may be challenging in some environments to build Compass with this dependency (see [below](#challenges-building-compass-due-to-ort-dependency)).

For cases where energy estimation is not required, the routee-compass CLI may be built without the routee-compass-powertrain dependency:

```sh
% cargo build -r --no-default-features -p routee-compass --manifest-path rust/Cargo.toml
```

## Challenges building Compass due to ort dependency

Some HPC systems run operating systems with a glibc that is too old for the prebuilt ONNX Runtime binaries that the Rust `ort` crate downloads by default (they require glibc >= 2.32; e.g. RHEL 8 ships glibc 2.28). On those systems, importing the compiled extension fails with an error like:

```
ImportError: .../routee_compass_py.cpython-313-x86_64-linux-gnu.so: undefined symbol: __libc_single_threaded
```

The fix is to build ONNX Runtime from source on the HPC machine and link against it. The pixi `hpc` environment automates this. 

> Note: This is designed for use via the Compass Python API.

### One-time setup

```bash
pixi run -e hpc build_hpc
```

This does two things:

1. Clones and builds ONNX Runtime from source into `build/onnxruntime/`
   (`scripts/hpc/build_ort.sh`; this takes 30+ minutes the first time).
2. Builds and installs the python package into the `hpc` environment with
   `maturin develop --uv --release`.

To verify the build imports correctly:

```bash
pixi run -e hpc check_hpc
```

### How it works

The `hpc` pixi feature's activation sets `ORT_LIB_PATH` to the local ONNX
Runtime build. The `ort` crate's build script prefers `ORT_LIB_PATH` over its
downloaded binaries, so *any* build run inside the `hpc` environment —
`build_py`, `build_rust`, `test`, `build_hpc_wheel` — automatically links the
locally-built ONNX Runtime. No special cargo features or flags are needed.

### Day-to-day usage on HPC

Always use the `hpc` environment. The easiest way is to activate it once per
shell session, after which plain commands work as usual:

```bash
pixi shell -e hpc
python -c "import nrel.routee.compass"   # works
pytest python/tests/
```

Or prefix individual commands with `-e hpc`:

```bash
pixi run -e hpc python          # run python with the package importable
pixi run -e hpc test            # run the python test suite
pixi run -e hpc build_rust      # build the rust CLI binaries
pixi run -e hpc build_hpc_wheel # build a distributable wheel (target/wheels/)
```

```{warning}
Do **not** run default-environment pixi commands (e.g. plain `pixi run python`
or `pixi install`) on an HPC machine with an old glibc. The default
environment installs this package as an editable install, and pixi/uv will
silently rebuild the extension against the downloaded ONNX Runtime binaries —
clobbering the working build and reintroducing the `__libc_single_threaded`
import error.
```

### Knobs

Environment variables read by the build (defaults set by the `hpc` feature in
`pyproject.toml`):

- `SKIP_ORT_BUILD=1` — reuse an existing ONNX Runtime build instead of
  rebuilding it (e.g. `SKIP_ORT_BUILD=1 pixi run -e hpc build_hpc`).
- `ONNXRUNTIME_TAG` — ONNX Runtime git tag to build (default `v1.20.1`).
- `ORT_BUILD_CONFIG` — CMake build config (default `RelWithDebInfo`).
- `ONNXRUNTIME_DIR` — where ONNX Runtime is cloned and built.
