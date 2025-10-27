use std::env;
use std::process;

fn main() {
    let current_dir = env::current_dir().unwrap();

    // Look for config file
    let config_path = current_dir.join("mikofia.config.json");

    if !config_path.exists() {
        eprintln!("❌ Config file not found: mikofia.config.json");
        eprintln!("   Create a config file in the current directory.");
        process::exit(2);
    }

    // Load config file
    let config = match mikofia::Config::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Failed to load config: {}", e);
            process::exit(2);
        }
    };

    println!("🚀 Running mikofia check...\n");

    // Run validation
    let violations = mikofia::check(&config.nodes, &current_dir);

    if violations.is_empty() {
        println!("✅ All checks passed!");
        process::exit(0);
    } else {
        println!("❌ {} violation(s) found:\n", violations.len());
        for (i, v) in violations.iter().enumerate() {
            println!("[{}] {}", i + 1, v.message);
        }
        process::exit(1);
    }
}
