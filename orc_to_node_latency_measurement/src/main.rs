use redis::Commands;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use tokio;
use std::process::Command;
use std::str;
use std::time::Duration;

#[derive(Deserialize)]
struct Config {
    rpi_ip: String,
    vm_ip: String
}

#[tokio::main]
async fn main() {
    let redis_client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut redis_conn = redis_client.get_connection().unwrap();

    let config_path = "nodes_addresses.json";

    let config = match parse_json(config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[ERROR] Error while reading config file: {}", e);
            return;
        }
    };

    let nodes = vec![
        ("VM_node", config.vm_ip),
        ("RPI_node", config.rpi_ip)
    ];

    loop {
        for (node_id, node_ip) in &nodes {


            let output = Command::new("ping")
                .arg("-c")          // Number of ICMP pkts
                .arg("1")         // Just one ICMP pkt
                .arg(node_ip)
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() {
                        let stdout = str::from_utf8(&output.stdout).unwrap_or("");
                        if let Some(latency) = parse_ping_output(stdout) {
                            // Pushing value to Redis. Key format: latency:<FROM>:<TO>
                            let redis_key = format!("latency:ORC:{}", node_id);
                            let _: () = redis_conn
                                .set(redis_key.clone(), latency)
                                .unwrap();

                            println!(
                                "[INFO] Latency measured: E-ORC ---- {:.2} ms ----> {}. Updated Redis key: {}", 
                                latency, 
                                node_id,
                                redis_key
                            );
                        } else {
                            println!("[WARNING] Could not parse ping output.")
                        }
                    } else {
                        eprintln!(
                            "[ERROR] Ping command failed with error: {}",
                            String::from_utf8_lossy(&output.stderr)
                        )
                    }
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to execute ping command: {}", e);
                }
            }
        }

        // Wait a little before the next measurement
        std::thread::sleep(Duration::from_secs(5))
    }
}


fn parse_ping_output(ping_output: &str) -> Option<f64> {
    ping_output
        .lines()
        .find(|line| line.contains("time="))        // Looking for the "time=" field in the ping output
        .and_then(|line| {
            line.split_whitespace()
                .find(|field| field.starts_with("time="))
                .and_then(|time_field| {
                    time_field
                        .trim_start_matches("time=")
                        .parse::<f64>()
                        .ok()       // Converting to f64 if valid   
                })
        })
}

fn parse_json(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

// Measure latency to a given address using ping
// fn measure_latency(ip: IpAddr) -> Option<f64> {
//     match ping(ip, Some(Duration::from_secs(1)), Some(1), None, None, None) {
//         Ok(duration) => Some(duration.as_secs_64() * 1000.0),      // Converting to ms
//         Err(_) => None,
//     }
// }
