use edgeless_function::*;
use serde::{Serialize, Deserialize};
use serde_json;

struct ClassifyFun;

impl EdgeFunction for ClassifyFun {

    // ------ EDGELESS FUNCTIONS REDEFINITION ------
    fn handle_cast(_src: InstanceId, encoded_message: &[u8]) {
        #[derive(Debug, Deserialize)]
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
            features: Features,
        }

        #[derive(Debug, Serialize)]
        enum Classification {
            LowActivity,
            HighActivity,
            Unknown,
        }

        #[derive(Debug, Serialize)]
        struct ClassificationPayload {
            batch_id: u64,
            classification: Classification,
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
        let extracted_features = received_data.features;

        let classification_result;
        if extracted_features.var_x < 30.0 && extracted_features.var_y < 30.0 && extracted_features.var_z < 30.0 {
            classification_result = Classification::LowActivity
        } else if extracted_features.var_x > 37.0 || extracted_features.var_y > 37.0 || extracted_features.var_z > 37.0 {
            classification_result = Classification::HighActivity
        } else {
            classification_result = Classification::Unknown
        }

        log::info!("Classified the received features");

        let payload = ClassificationPayload {
            batch_id,
            classification: classification_result,
        };

        let serialized_classification_result = match serde_json::to_string(&payload) {
            Ok(json) => json,
            Err(e) => {
                log::info!("Error serializing classification result: {}", e);
                String::new()
            }
        };

        cast("classification_result", serialized_classification_result.as_bytes());
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

edgeless_function::export!(ClassifyFun);
