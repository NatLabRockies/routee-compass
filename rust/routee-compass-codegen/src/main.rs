use clap::{Parser, Subcommand};
use routee_compass_codegen::generator::traversal::TraversalExtensions;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cargo-compass")]
#[command(bin_name = "cargo")]
#[command(about = "Code generation tools for RouteE Compass plugin development")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "compass")]
    Compass(CompassArgs),
}

#[derive(Parser)]
struct CompassArgs {
    #[command(subcommand)]
    subcommand: CompassSubcommands,
}

#[derive(Subcommand)]
enum CompassSubcommands {
    /// Generate a new TraversalModel module
    Traversal {
        /// Name of the traversal model in PascalCase (e.g., EnergyCost)
        name: String,
        /// Parent directory path to where the module should be created (e.g., src)
        path: PathBuf,
        /// optionally include extensions for typed configuration and engine struct
        #[arg(long)]
        extensions: Option<TraversalExtensions>,
        /// allow the user to force overwriting existing files
        #[arg(short, long)]
        force: bool
    },
    /// Generate a new ConstraintModel module
    Constraint {
        /// Name of the constraint model in PascalCase (e.g., DistanceLimit)
        name: String,
        /// Parent directory path to where the module should be created (e.g., src)
        path: PathBuf,
    },
    /// Generate a new InputPlugin module
    InputPlugin {
        /// Name of the input plugin in PascalCase (e.g., CustomLoader)
        name: String,
        /// Parent directory path to where the module should be created (e.g., src)
        path: PathBuf,
    },
    /// Generate a new OutputPlugin module
    OutputPlugin {
        /// Name of the output plugin in PascalCase (e.g., CustomFormatter)
        name: String,
        /// Parent directory path to where the module should be created (e.g., src)
        path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Cli {
        command: Commands::Compass(args),
    } = Cli::parse();

    match args.subcommand {
        CompassSubcommands::Traversal {
            name,
            path,
            extensions,
            force
        } => {

            routee_compass_codegen::generator::traversal::generate_traversal_module(
                &name,
                &path,
                extensions.as_ref(),
                force
            )?;
        }
        CompassSubcommands::Constraint { name, path } => {

            routee_compass_codegen::generator::constraint::generate_constraint_module(
                &name,
                &path,
            )?;
        }
        CompassSubcommands::InputPlugin { name, path } => {

            routee_compass_codegen::generator::input_plugin::generate_input_plugin_module(
                &name,
                &path,
            )?;
        }
        CompassSubcommands::OutputPlugin { name, path } => {

            routee_compass_codegen::generator::output_plugin::generate_output_plugin_module(
                &name,
                &path,
            )?;
        }
    }

    Ok(())
}
