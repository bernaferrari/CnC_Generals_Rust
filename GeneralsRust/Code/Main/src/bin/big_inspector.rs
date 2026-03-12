use generals_main::assets::big_file::BIGFile;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <big_file_path>", args[0]);
        return Ok(());
    }

    let big_file_path = &args[1];
    println!("Loading BIG file: {}", big_file_path);

    let mut big_file = BIGFile::new();
    big_file.open(big_file_path).await?;

    let file_list = big_file.list_files();

    println!("Files in BIG archive ({} total):", file_list.len());
    println!("{}", "=".repeat(80));

    for (i, filename) in file_list.iter().enumerate() {
        println!("{:4}: {}", i + 1, filename);

        // Show a sample of .wnd files (UI layout files)
        if filename.ends_with(".wnd") && i < 5 {
            println!("    -> UI Layout File");

            // Try to extract and show first few bytes
            if let Ok(data) = big_file.extract_file(filename).await {
                println!("    -> Size: {} bytes", data.len());

                // Show first 200 characters as text if it looks like text
                if let Ok(text) = std::str::from_utf8(&data[0..std::cmp::min(200, data.len())]) {
                    if text
                        .chars()
                        .all(|c| c.is_ascii() && (c.is_ascii_graphic() || c.is_whitespace()))
                    {
                        println!(
                            "    -> Preview: {}",
                            text.replace('\n', "\\n").replace('\r', "\\r")
                        );
                    }
                }
            }
            println!();
        }
    }

    // Count file types
    let mut extensions: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for filename in &file_list {
        if let Some(ext) = filename.split('.').next_back() {
            *extensions.entry(ext.to_lowercase()).or_insert(0) += 1;
        } else {
            *extensions.entry("no_extension".to_string()).or_insert(0) += 1;
        }
    }

    println!("\nFile types:");
    println!("{}", "=".repeat(40));
    for (ext, count) in extensions {
        println!("{:15}: {:4}", ext, count);
    }

    Ok(())
}
