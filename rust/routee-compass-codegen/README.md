# routee-compass-codegen

Code generation tools for RouteE Compass plugin development.

## Installation

```bash
cargo install --path rust/routee-compass-codegen
```

## Usage

### Traversal Models

Builds out a traversal model stub. Optionally include typed configuration and query parameters (`typed-config`) and a core engine type owned by the Service and shared to the model to separate API integration from business logic (`typed-config-and-engine`). This is all described in the --help menu for the command:

```
% cargo compass traversal --help                                                                 
Generate a new TraversalModel module

Usage: cargo compass traversal [OPTIONS] <NAME> <PATH>

Arguments:
  <NAME>
          Name of the traversal model in PascalCase (e.g., EnergyCost)

  <PATH>
          Parent directory path to where the module should be created (e.g., src)

Options:
      --extensions <EXTENSIONS>
          optionally include extensions for typed configuration and engine struct

          Possible values:
          - typed-config:            include the config.rs and params.rs files and deserialize the inputs to builder and service .build() methods into these types
          - typed-config-and-engine: also include an engine.rs file for module business logic with a TryFrom<Config> implementation stub

  -h, --help
          Print help (see a summary with '-h')
```

##### Example query

build out a traversal model at rust/src/model/traversal called FancyRobotModel. use the typed-config extension.

```
% cargo compass traversal FancyRobot rust/src/model/traversal --extensions typed-config 
```
