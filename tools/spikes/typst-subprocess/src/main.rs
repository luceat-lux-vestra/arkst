use std::process::Command;
use std::time::Instant;
use std::fs;

fn main() {
    println!("=== Typst Subprocess Comparison ===");
    
    // Test Case 1: Simple rect
    let simple_source = r#"
#set page(width: 100pt, height: 100pt)
#rect(width: 20pt, height: 20pt)
"#;
    
    fs::write("/tmp/simple.typ", simple_source).unwrap();
    
    println!("=== Test Case 1: Simple rect (no fonts) ===");
    let start = Instant::now();
    let output = Command::new("typst")
        .args(["compile", "/tmp/simple.typ", "/tmp/simple.pdf"])
        .output()
        .expect("Failed to run typst");
    let elapsed = start.elapsed();
    
    if output.status.success() {
        println!("✅ Success!");
        println!("  Time: {:.2?}", elapsed);
    } else {
        println!("❌ Failed!");
        println!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    println!();
    
    // Test Case 2: Text with font
    let text_source = r#"
#set page(width: 200pt, height: 100pt)
Hello from Arkst.
"#;
    
    fs::write("/tmp/text.typ", text_source).unwrap();
    
    println!("=== Test Case 2: Text with font ===");
    let start = Instant::now();
    let output = Command::new("typst")
        .args(["compile", "/tmp/text.typ", "/tmp/text.pdf"])
        .output()
        .expect("Failed to run typst");
    let elapsed = start.elapsed();
    
    if output.status.success() {
        println!("✅ Success!");
        println!("  Time: {:.2?}", elapsed);
    } else {
        println!("❌ Failed!");
        println!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    println!();
    
    // Test Case 3: Error fixture
    let error_source = r#"
#unknown-function()
"#;
    
    fs::write("/tmp/error.typ", error_source).unwrap();
    
    println!("=== Test Case 3: Invalid function call ===");
    let start = Instant::now();
    let output = Command::new("typst")
        .args(["compile", "/tmp/error.typ", "/tmp/error.pdf"])
        .output()
        .expect("Failed to run typst");
    let elapsed = start.elapsed();
    
    if output.status.success() {
        println!("⚠️ Unexpected success!");
    } else {
        println!("✅ Expected failure!");
        println!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
        println!("  Time: {:.2?}", elapsed);
    }
    
    println!();
    println!("=== Subprocess comparison completed ===");
}
