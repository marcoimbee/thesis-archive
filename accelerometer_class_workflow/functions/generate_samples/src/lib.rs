use edgeless_function::*;
use std::{thread, time::Duration};
use serde::Serialize;
use serde_json;
struct GenerateSamplesFun;

impl EdgeFunction for GenerateSamplesFun {

    // ------ EDGELESS FUNCTIONS REDEFINITION ------
    fn handle_cast(_src: InstanceId, _encoded_message: &[u8]) {
        log::info!("generate_samples: handle_cast() called");
    }

    fn handle_call(_src: InstanceId, _encoded_message: &[u8]) -> CallRet {
        log::info!("generate_samples: handle_call() called");
        CallRet::NoReply
    }

    // Starts and generates 100 samples every 5 seconds. 
    // Sends them to the next function in the workflow
    fn handle_init(_payload: Option<&[u8]>, _init_state: Option<&[u8]>) {
        edgeless_function::init_logger();
        log::info!("GENERATE_SAMPLES: started");

        let batch_size: usize = 100;
        let interval_seconds: u64 = 5;
        let mut counter: i32 = 0;

        #[derive(Debug, Serialize)]
        struct AccelerometerData {
            x: f64,
            y: f64,
            z: f64,
        }

        log::info!("Starting generation...");
        loop {
            counter += 1;
    
            let mut batch = Vec::with_capacity(batch_size);

            for _ in 0..batch_size {                // Generating batch of accelerometric data
                batch.push(
                    AccelerometerData {         // TODO: find a way to generate randvalues here
                        x: 12.67,
                        y: 7.23,
                        z: 56.1,
                    }
                );
            }

            println!("[INFO] Generated batch {}", counter);
            println!("\n");

            let serailized_batch = match serde_json::to_string(&batch) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("Error serializing accelerometric data batch: {}", e);
                    String::new()
                }
            };

            cast("generated_samples", serailized_batch.as_bytes());

            thread::sleep(Duration::from_secs(interval_seconds));
        }
    }

    fn handle_stop() {
        log::info!("GENERATE_SAMPLES: stopped");
    }
}

edgeless_function::export!(GenerateSamplesFun);