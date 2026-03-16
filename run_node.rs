use std::fs;

fn main() {
    let output = std::process::Command::new("cargo")
        .args(&["run", "--bin", "quanta", "--", "start", "--network", "testnet", "--no-network"])
        .output()
        .expect("Failed to execute cargo");

    println!("Output: {}", String::from_utf8_lossy(&output.stdout));
    println!("Err: {}", String::from_utf8_lossy(&output.stderr));
}
