use edgeless_function::*;
use serde::{Serialize, Deserialize};
use serde_json;

struct ExtractFeaturesFun;

impl EdgeFunction for ExtractFeaturesFun {

    // ------ EDGELESS FUNCTIONS REDEFINITION ------
    fn handle_cast(_src: InstanceId, encoded_message: &[u8]) {
        #[derive(Debug, Deserialize)]
        struct AccelerometerData {
            x: f64,
            y: f64,
            z: f64,
        }

        #[derive(Debug, Serialize)]
        struct Features {
            mean_x: f64,
            mean_y: f64,
            mean_z: f64,
            var_x: f64,
            var_y: f64,
            var_z: f64,
        }

        #[derive(Debug, Deserialize)]
        struct ReceivedPayload {
            batch_id: u64,
            batch: Vec<AccelerometerData>,
        }

        #[derive(Debug, Serialize)]
        struct FeaturesPayload {
            batch_id: u64,
            features: Features,
        }

        let str_message = core::str::from_utf8(encoded_message).unwrap();
        let received_data: ReceivedPayload = match serde_json::from_str(str_message) {
            Ok(parsed_received_data) => parsed_received_data,
            Err(err) => {
                log::info!("Failed to deserialize message: {}", err);
                return;
            }
        };

        let batch_id = received_data.batch_id;
        let accelerometer_data = received_data.batch;

        // Feature extraction
        let batch_size = accelerometer_data.len() as f64;

        let (sum_x, sum_y, sum_z): (f64, f64, f64) = accelerometer_data.iter().fold((0.0, 0.0, 0.0), |acc, data| {
            (acc.0 + data.x, acc.1 + data.y, acc.2 + data.z)
        });

        let mean_x = sum_x / batch_size;
        let mean_y = sum_y / batch_size;
        let mean_z = sum_z / batch_size;

        let (var_x, var_y, var_z): (f64, f64, f64) = accelerometer_data.iter().fold((0.0, 0.0, 0.0), |acc, data| {
            (
                acc.0 + (data.x - mean_x).powi(2),
                acc.1 + (data.y - mean_y).powi(2),
                acc.2 + (data.z - mean_z).powi(2),
            )
        });

        let features = Features {
            mean_x,
            mean_y,
            mean_z,
            var_x: var_x / batch_size,
            var_y: var_y / batch_size,
            var_z: var_z / batch_size,
        };

        log::info!("Features have been extracted");

        let payload = FeaturesPayload {
            batch_id,
            features,
        };

        let serialized_features = match serde_json::to_string(&payload) {
            Ok(json) => json,
            Err(e) => {
                log::info!("Error serializing extracted features: {}", e);
                String::new()
            }
        };

        cast("extracted_features", serialized_features.as_bytes());
    }

    fn handle_call(_src: InstanceId, _encoded_message: &[u8]) -> CallRet {
        log::info!("handle_call() called");
        CallRet::NoReply
    }

    fn handle_init(_payload: Option<&[u8]>, _init_state: Option<&[u8]>) {
        edgeless_function::init_logger();
        log::info!("started");

    }

    fn handle_stop() {
        log::info!("stopped");
    }
}

edgeless_function::export!(ExtractFeaturesFun);