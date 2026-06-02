use clap::{Parser, Subcommand};
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
        /// Comma-delimiited list of files to copy over. by default, copy all files.
        #[arg(
            long,
            default_value = "builder.rs,config.rs,engine.rs,mod.rs,model.rs,params.rs,service.rs"
        )]
        files: String,
        /// allow the user to force overwriting existing files
        #[arg(short, long)]
        force: bool,
    },
    /// Generate a new ConstraintModel module
    Constraint {
        /// Name of the constraint model in PascalCase (e.g., DistanceLimit)
        name: String,
        /// Parent directory path to where the module should be created (e.g., src)
        path: PathBuf,
        /// Comma-delimiited list of files to copy over. by default, copy all files.
        #[arg(
            long,
            default_value = "builder.rs,config.rs,engine.rs,mod.rs,model.rs,params.rs,service.rs"
        )]
        files: String,
        /// allow the user to force overwriting existing files
        #[arg(short, long)]
        force: bool,
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
            files,
            force,
        } => {
            let files: Vec<String> = files.split(",").map(String::from).collect();
            routee_compass_codegen::generator::util::generate_module(
                routee_compass_codegen::generator::CodegenComponentType::Traversal,
                &files,
                &name,
                &path,
                force,
            )?;
        }
        CompassSubcommands::Constraint {
            name,
            path,
            files,
            force,
        } => {
            let files: Vec<String> = files.split(",").map(String::from).collect();
            routee_compass_codegen::generator::util::generate_module(
                routee_compass_codegen::generator::CodegenComponentType::Constraint,
                &files,
                &name,
                &path,
                force,
            )?;
        }
        CompassSubcommands::InputPlugin { name: _, path: _ } => {
            todo!()
        }
        CompassSubcommands::OutputPlugin { name: _, path: _ } => {
            todo!()
        }
    }

    Ok(())
}
