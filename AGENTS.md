# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**RouteE Compass** 
RouteE Compass is an energy-aware routing engine for the RouteE ecosystem of software tools with the following key features:

    - Dynamic and extensible search objectives that allow customized blends of distance, time, cost, and energy (via RouteE Powertrain) at query-time
    - Core engine written in Rust for improved runtimes, parallel query execution, and the ability to load nation-sized road networks into memory
    - Rust and Python APIs for integration into different research pipelines and other software

## Repository Structure

```
docs/                       # Documentation source files
examples/                   # Example notebooks and scripts demonstrating usage
python/                     # Python wrapper code
  nrel/routee/compass/      # Main Python package source
rust/                       # Core Rust implementation
  routee-compass/           # Main application and binary crate
  routee-compass-core/      # Core data structures, graph algorithms, and search logic
  routee-compass-macros/    # Procedural macros for the project
  routee-compass-powertrain/# Integration with RouteE Powertrain models
  routee-compass-py/        # Rust-Python bindings using PyO3
scripts/                    # Utility scripts for building and maintenance
```

## Code Quality Requirements

**All code changes must pass the following checks before being committed:**

```bash
# Rust
cargo test
cargo fmt -- --check                                  # Rust formatting
cargo clippy --all --all-targets --all-features -- -D warnings  # Rust linting

# Python
ruff check
ruff format --check
mypy python
pytest python
```

## Component-Specific Guidance

**For Rust core development**:
- All core rust code is in `rust/`

**For Python Wrapper development**:
- Package is managed with matruin and pyproject.toml and uses PyO3 bindings
- Development setup: `pip install -e .[all]` from root directory

## Build Commands

### Rust 
```bash
cd rust 

# Build 
cargo build --release
```

### Python Wrapper 
```bash
conda activate routee-compass
maturin develop
```
