# CLAUDE.md

This file provides guidance to AI agents when working with code in this repository.

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
pixi run check_all    # Run all checks (Python + Rust)
pixi run check_py     # Python only: ruff format, ruff lint, mypy, pytest
pixi run check_rust   # Rust only: cargo fmt, cargo clippy, cargo test
```

## Build Commands

### Rust 
```bash
pixi run build_rust
```

### Python Wrapper 
```bash
pixi run build_py
```
