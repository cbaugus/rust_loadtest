use rust_loadtest::config_docs_generator::ConfigDocsGenerator;
use std::fs;

fn main() {
    println!("Generating configuration documentation...\n");

    let generator = ConfigDocsGenerator::new();

    // Generate JSON Schema
    println!("1. Generating JSON Schema...");
    let schema = generator.generate_json_schema();
    fs::write("docs/config-schema.json", &schema).expect("Failed to write JSON Schema");
    println!("   ✅ Saved to docs/config-schema.json ({} bytes)", schema.len());

    // Generate Markdown documentation
    println!("2. Generating Markdown documentation...");
    let markdown = generator.generate_markdown_docs();
    fs::write("docs/CONFIG_SCHEMA.md", &markdown).expect("Failed to write Markdown docs");
    println!("   ✅ Saved to docs/CONFIG_SCHEMA.md ({} bytes)", markdown.len());

    // Generate VS Code snippets
    println!("3. Generating VS Code snippets...");
    let snippets = generator.generate_vscode_snippets();
    fs::create_dir_all(".vscode").ok();
    fs::write(".vscode/rust-loadtest.code-snippets", &snippets)
        .expect("Failed to write VS Code snippets");
    println!("   ✅ Saved to .vscode/rust-loadtest.code-snippets ({} bytes)", snippets.len());

    println!("\n✅ All documentation generated successfully!");
}
