use std::fs;
use std::path::Path;

use super::CodegenComponentType;

use heck::ToSnakeCase;

/// creates the file contents and writes to the files with template code.
pub fn generate_traversal_module(
    pascal_case_name: &str,
    path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent_traversal_in_path = path.to_str().map(|p| p.contains("..")).unwrap_or_default();
    if parent_traversal_in_path {
        return Err("provided path traverses upward with '..' which is not allowed".into());
    }

    let snake_case_name = pascal_case_name.to_snake_case();
    let module_dir = path.join(&snake_case_name);
    fs::create_dir_all(&module_dir)?;

    let files = [
        "builder.rs",
        "config.rs",
        "engine.rs",
        "mod.rs",
        "model.rs",
        "params.rs",
        "service.rs",
    ];

    for file in files.iter() {
        let temp_base = CodegenComponentType::Traversal.retrieve_template(file)?;
        let temp_mod = temp_base.replace("Template", pascal_case_name);
        super::util::write_file(module_dir.join(file).as_path(), temp_mod, force)?
    }

    println!(
        "✓ Generated {} module at {}/{}",
        pascal_case_name,
        path.display(),
        snake_case_name
    );
    println!("  Next steps:");
    println!("  1. Add 'mod {};' in the correct file (mod.rs/lib.rs) in the parent directory of your target", snake_case_name);
    println!("  2. Implement the trait methods in each file");
    println!("  3. Register builder with inventory::submit! in your plugin registration");

    Ok(())
}
