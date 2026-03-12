use generals_main::game_logic::script_loader;

fn main() {
    let map = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "BarrenBadlands".to_string());

    match script_loader::load_chunky_map(&map) {
        Ok(Some(chunky)) => {
            let mut labels: Vec<String> = chunky.toc.values().cloned().collect();
            labels.sort();
            labels.dedup();

            println!("map={}", map);
            println!("source={}", chunky.source.display());
            println!("toc_entries={}", chunky.toc.len());
            for label in labels {
                println!("{label}");
            }
        }
        Ok(None) => {
            eprintln!("No map file found for '{map}'");
            std::process::exit(2);
        }
        Err(err) => {
            eprintln!("Failed to parse '{map}': {err}");
            std::process::exit(1);
        }
    }
}
