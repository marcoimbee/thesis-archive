use edgeless_function::*;
use serde::Serialize;
use serde_json;

struct GenerateSamplesFun;

struct InitState {                  // Populated by what has been specified into "init-payload" in workflow.json
    batch_size: u64,
    generation_interval_ms: u64,
}

struct State {
    counter: u64,
}

static INIT_STATE: std::sync::OnceLock<InitState> = std::sync::OnceLock::new();
static STATE: std::sync::OnceLock<std::sync::Mutex<State>> = std::sync::OnceLock::new();

impl EdgeFunction for GenerateSamplesFun {

    // ------ EDGELESS FUNCTIONS REDEFINITION ------

    // Starts and generates 100 samples every 5 seconds. 
    // Sends them to the next function in the workflow
    fn handle_cast(_src: InstanceId, _encoded_message: &[u8]) {
        let init_state = INIT_STATE.get().unwrap();         // Getting initialization params
        let mut state = STATE.get().unwrap().lock().unwrap();

        let batch_size = init_state.batch_size;
        let generation_interval_ms: u64 = init_state.generation_interval_ms;

        #[derive(Debug, Serialize)]
        struct AccelerometerData {
            x: f64,
            y: f64,
            z: f64,
        }

        #[derive(Debug, Serialize)]
        struct Payload {
            batch_id: u64,
            batch: Vec<AccelerometerData>,
        }

        if state.counter == 0 {
            cast("aoi_measurement_start", "--- NEW MEASUREMENT ROUND ---".as_bytes());
        }
    
        let mut batch = Vec::with_capacity(batch_size.try_into().unwrap());

        for _ in 0..batch_size.try_into().unwrap() {                // Generating batch of accelerometric data
            batch.push(
                AccelerometerData {
                    x: 12.67,
                    y: 7.23,
                    z: 56.1,
                }
            );
        }

        log::info!("Generated batch #{}", state.counter);

        let payload = Payload {
            batch_id: state.counter,
            batch,
        };

        let serialized_payload = match serde_json::to_string(&payload) {
            Ok(json) => json,
            Err(e) => {
                log::info!("Error serializing payload: {}", e);
                String::new()
            }
        };

        // Forwarding data in the workflow
        cast("generated_samples", serialized_payload.as_bytes());

        // Forwarding to file-log resource
        cast("aoi_measurement_start", format!("{}", state.counter).as_bytes());

        state.counter += 1;
        delayed_cast(generation_interval_ms, "self", b"");
    }

    fn handle_call(_src: InstanceId, _encoded_message: &[u8]) -> CallRet {
        log::info!("handle_call() called");
        CallRet::NoReply
    }

    fn handle_init(payload: Option<&[u8]>, _init_state: Option<&[u8]>) {
        edgeless_function::init_logger();

        let arguments = if let Some(payload) = payload {
            let str_payload = core::str::from_utf8(payload).unwrap();
            edgeless_function::parse_init_payload(str_payload)
        } else {
            std::collections::HashMap::new()
        };

        let batch_size = arguments.get("batch_size").expect("Invalid batch size provided").parse::<u64>().unwrap();
        let generation_interval_ms = arguments.get("generation_interval_ms").expect("Invalid generation interval provided").parse::<u64>().unwrap();

        let _ = INIT_STATE.set(
            InitState { 
                batch_size, 
                generation_interval_ms
            }
        );

        let _ = STATE.set(std::sync::Mutex::new(State { counter: 0 }));

        log::info!(
            "Started. Batch size: {}, Generation interval (ms): {}", 
            batch_size, 
            generation_interval_ms
        );

        delayed_cast(5000, "self", b"");          // Action happens in handle_cast()
    }

    fn handle_stop() {
        log::info!("Stopped");
    }
}

edgeless_function::export!(GenerateSamplesFun);
