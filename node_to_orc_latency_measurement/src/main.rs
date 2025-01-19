use redis::Commands;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use tokio;
use std::process::Command;
use std::str;
use std::time::Duration;

#[derive(Deserialize)]
struct NodeConfig {
    uuid: String,
    ip_address: String
}


#[derive(Deserialize)]
struct NodeData {
    vm_node: Option<NodeConfig>,
    rpi_node: Option<NodeConfig>,
}

#[derive(Deserialize)]
struct Config {
    redis_server_ip_address: String,
    nodes_data: Vec<NodeData>,
}

#[tokio::main]
async fn main() {
    let config_path = "config.json";
    let config = match parse_json(config_path) {
        Ok(config) => config,
        Err(e) => {
        eprintln!("[ERROR] Error while reading config file: {}", e);
            return;
        }
    };

    let redis_client = redis::Client::open(format!("redis://{}/", config.redis_server_ip_address)).unwrap();
    let mut redis_conn = redis_client.get_connection().unwrap();

    let config = match parse_json(config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[ERROR] Error while reading config file: {}", e);
            return;
        }
    };

    let nodes = config 
        .nodes_data
        .iter()
        .flat_map(|node_data| {
            let mut nodes = Vec::new();
            if let Some(vm_node) = &node_data.vm_node {
                nodes.push(("vm_node", vm_node.uuid.clone(), vm_node.ip_address.clone()));
            }
            if let Some(rpi_node) = &node_data.rpi_node {
                nodes.push(("rpi_node", rpi_node.uuid.clone(), rpi_node.ip_address.clone()));
            }
            nodes
        })
        .collect::<Vec<_>>();

    loop {
        for (_, node_uuid, node_ip) in &nodes {
            let output = Command::new("ping")
                .arg("-c")          // Number of ICMP pkts
                .arg("1")           // Just one ICMP pkt
                .arg(node_ip)
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() {
                        let stdout = str::from_utf8(&output.stdout).unwrap_or("");
                        if let Some(latency) = parse_ping_output(stdout) {
                            // Pushing value to Redis 
                            // Key format: latency:<uuid_source_node>:orc
                            let redis_key = format!("latency:{}:orc", node_uuid);
                            let _: () = redis_conn
                                .set(redis_key.clone(), latency)
                                .unwrap();

                            println!(
                                "[INFO] Latency measured: {} ---- {:.2} ms ----> E-ORC. Updated Redis key: {}", 
                                node_uuid,
                                latency,
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
