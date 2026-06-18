# Troubleshooting MacOS Installation

If you are having installation problems on MacOS when installing the Rust dependencies for `routee-compass` via `cargo build --release`, please read through the following: 

## Problem
The MacOS linker can create issues when trying to link Python binaries with Rust's Python wrapper, PyO3. This occurs because PyO3 leaves Python symbols unresolved until runtime, but the macOS linker is incredibly strict and rejects any undefined symbols during compilation.

E.g., for `arm64`, the build error will look like the following:
```text
ld: symbol(s) not found for architecture arm64
clang: error: linker command failed with exit code 1 (use -v to see invocation)
error: could not compile `routee-compass-py` (lib) due to 1 previous error
```

## Fix

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
