use libretune_core::ini::EcuDefinition;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: parse_test <ini_file>");
        return;
    }

    let path = &args[1];
    println!("Parsing: {}", path);

    match EcuDefinition::from_file(path) {
        Ok(def) => {
            println!("Successfully parsed INI!");
            println!("Signature: {}", def.signature);
            println!("Output Channels: {}", def.output_channels.len());
            println!("Constants: {}", def.constants.len());
            println!("Tables: {}", def.tables.len());
            println!("Gauges: {}", def.gauges.len());
            
            println!("\nTables:");
            let mut table_names: Vec<_> = def.tables.keys().collect();
            table_names.sort();
            for name in table_names.iter().take(5) {
                let tbl = &def.tables[*name];
                println!("  - {}: title=\"{}\", map={}, x_bins={}, y_bins={:?}", 
                    name, tbl.title, tbl.map, tbl.x_bins, tbl.y_bins);
            }

            // Print a few channels as a sample
            println!("\nSample Output Channels:");
            let mut channels: Vec<_> = def.output_channels.keys().collect();
            channels.sort();
            for name in channels.iter().take(10) {
                let ch = &def.output_channels[*name];
                println!("  - {}: offset={}, units={}", name, ch.offset, ch.units);
            }
        }
        Err(e) => {
            eprintln!("Failed to parse INI: {:?}", e);
            std::process::exit(1);
        }
    }
}
