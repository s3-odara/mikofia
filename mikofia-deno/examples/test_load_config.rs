use mikofia_deno::load_deno_config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load the Deno config file (JavaScript or TypeScript)
    let config_path = std::env::current_dir()?
        .parent()
        .ok_or("No parent directory")?
        .join("mikofia.config.js");

    println!("Loading config from: {:?}", config_path);

    let config = load_deno_config(&config_path).await?;

    println!("✓ Config loaded successfully!");
    println!("  Total nodes: {}", config.nodes.len());

    for (i, node) in config.nodes.iter().enumerate() {
        println!(
            "  [{}] path: {}, existence: {:?}, kind: {:?}",
            i, node.path, node.existence, node.kind
        );

        if !node.children.is_empty() {
            println!("      children: {}", node.children.len());
        }

        if node.strict.is_some() {
            println!("      strict: {:?}", node.strict);
        }
    }

    Ok(())
}
