use clap::Parser;
use crossterm::{execute, terminal::ClearType};
use crossterm::terminal::Clear;
use std::io::{self, stdout, Write};
use serde_json::Value;
use std::fs;
use ctrlc;

#[derive(Debug, clap::Parser)]
#[command(long_about = None)]
struct Args {
    #[arg(short = 'p', long, default_value_t = String::from("Redis"))]
    proxy_type: String,
    #[arg(short = 'r', long, default_value_t = String::from("redis://localhost:6379"))]
    redis_url: String,
    #[arg(short = 'w', long, default_value_t = 5)]
    wait_interval: u64,
    #[arg(short = 'l', long, default_value_t = 100.0)]
    latency_threshold: f64,
    #[arg(short = 'm', long, default_value_t = String::from("single"))]
    relocation_mode: String,
}

fn show_header(choice: &str) {
    let ascii_art_1 = r#"
           _           _                  _             
          | |         | |                (_)            
  _ __ ___| |__   __ _| | __ _ _ __   ___ _ _ __   __ _ 
 | '__/ _ \ '_ \ / _` | |/ _` | '_ \ / __| | '_ \ / _` |
 | | |  __/ |_) | (_| | | (_| | | | | (__| | | | | (_| |
 |_|  \___|_.__/ \__,_|_|\__,_|_| |_|\___|_|_| |_|\__, | o o o
                                                   __/ |
                                                  |___/ 
"#;

    let ascii_art_2 = r#"
                           _  _                _                        
                          (_)| |              (_)                       
  _ __ ___    ___   _ __   _ | |_  ___   _ __  _  _ __    __ _          
 | '_ ` _ \  / _ \ | '_ \ | || __|/ _ \ | '__|| || '_ \  / _` |         
 | | | | | || (_) || | | || || |_| (_) || |   | || | | || (_| | 
 |_| |_| |_| \___/ |_| |_||_| \__|\___/ |_|   |_||_| |_| \__, | o o o
                                                          __/ |         
                                                         |___/          
"#;

    if choice == "1" { println!("{}", ascii_art_1); }
    else { println!("{}", ascii_art_2); }
}

fn show_menu() {
    println!();
    println!("--- Options: ---");
    println!("1. Start the Network Aware Orchestrator (migrate)");
    println!("2. Monitor cluster with Network Aware Orchestrator (do not migrate)");
    println!("3. Move everything on the RPI node");
}

fn show_nodes_names() {
    let json_data = fs::read_to_string("config.json").expect("Failed to read JSON config file");
    let parsed: serde_json::Value = serde_json::from_str(&json_data).expect("Failed to parse JSON config file");
    println!("RPI_node: {}", parsed["RPI_node_UUID"]);
    println!("VM_node: {}", parsed["VM_node_UUID"]);
    println!();
}

fn clear_console() {
    execute!(stdout(), Clear(ClearType::All)).unwrap();
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    anyhow::ensure!(
        args.proxy_type.to_lowercase() == "redis",
        "unknown proxy type: {}",
        args.proxy_type
    );

    let mut net_aware_orc = delegated_orc::network_aware_orchestrator::NetworkAwareOrchestrator::new(
        &args.redis_url,
        args.latency_threshold,
        &args.relocation_mode
    )?;

    let mut counter = 0;

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    ctrlc::set_handler(move || {
        tx.send(()).unwrap();
    })?;

    loop {
        show_menu();

        println!("Choose an option: ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .expect("Failed to read line");
        let choice = choice.trim();

        match choice {
            "1" => {
                loop {
                    show_header(choice);
                    show_nodes_names();

                    let start_time = std::time::Instant::now();

                    counter += 1;
                    println!("[INFO] {}-th iteration.", &counter);

                    let (done, num_migrations) = net_aware_orc.rebalance();
                    if done {
                        println!(
                            "{} migrations, redistribution time: {} ms",
                            num_migrations,
                            start_time.elapsed().as_millis()
                        );
                    }

                    if rx.try_recv().is_ok() {
                        break;
                    }

                    std::thread::sleep(std::time::Duration::from_secs(args.wait_interval));

                    clear_console();
                }
            },

            "2" => {
                loop {
                    show_header(choice);
                    show_nodes_names();

                    counter += 1;
                    println!("[INFO] {}-th iteration.", &counter);

                    net_aware_orc.monitor_cluster();

                    if rx.try_recv().is_ok() {
                        break;
                    }

                    std::thread::sleep(std::time::Duration::from_millis(2000));

                    clear_console();
                }
            },

            "3" => {
                println!();

                let json_data = fs::read_to_string("config.json").expect("Failed to read JSON config file");
                let parsed: Value = serde_json::from_str(&json_data).expect("Failed to parse JSON config file");
                let rpi_node_id = parsed
                    .get("RPI_node_UUID")
                    .and_then(|v| v.as_str())
                    .expect("RPI_node_uuid not found or not a string");

                let (done, moved_funcs) = net_aware_orc.move_to_rpi(rpi_node_id);
                if done {
                    println!("[INFO] Successfully moved {} functions to the RPI node.", moved_funcs);
                } else {
                    eprintln!("[ERROR] An error occurred.");
                }
            },

            &_ => todo!(),
        }
    }
}