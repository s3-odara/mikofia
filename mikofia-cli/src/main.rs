use std::env;

use mikofia::{Existence, Node};

fn main() {
    let current_dir = env::current_dir().unwrap();

    let nodes = vec![
        Node {
            path: "Cargo.toml".to_string(),
            existence: Existence::Required,
        },
        Node {
            path: "README.md".to_string(),
            existence: Existence::Optional,
        },
        Node {
            path: "temp".to_string(),
            existence: Existence::Absent,
        },
    ];

    let violations = mikofia::check(&nodes, &current_dir);

    if violations.is_empty() {
        println!("✅ All checks passed!");
        std::process::exit(0);
    } else {
        println!("❌ {} violation(s) found:\n", violations.len());
        for (i, v) in violations.iter().enumerate() {
            println!("[{}] {}", i + 1, v.message);
        }
        std::process::exit(1);
    }
}
