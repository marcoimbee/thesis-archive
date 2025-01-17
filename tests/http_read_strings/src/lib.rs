use edgeless_function::*;
use edgeless_http::*;
use serde_json::{json, Value};

struct HttpReadStringsFun;

impl EdgeFunction for HttpReadStringsFun {
    fn handle_cast(_src: InstanceId, _encoded_message: &[u8]) {}

    fn handle_call(_src: InstanceId, encoded_message: &[u8]) -> CallRet {
        let str_message = core::str::from_utf8(encoded_message).unwrap();

        log::info!("http_read_strings: 'Call' called, MSG: {}", str_message);
        let req: EdgelessHTTPRequest = edgeless_http::request_from_string(&str_message).unwrap();

        let res_params = if req.path == "/read_strings" {
            if let Some(body) = req.body {
                if let Ok(content) = String::from_utf8(body) {
                    // Parse body as JSON
                    match serde_json::from_str::<Value>(&content) {
                        Ok(json) => {
                            if let (Some(text), Some(word)) = (
                                json.get("Text").and_then(|v| v.as_str()),
                                json.get("Word").and_then(|v| v.as_str()),
                            ) {
                                // Log strings
                                log::info!("Received: Text: {}, Word: {}", text, word);

                                // Create a new JSON object
                                let response_json = json!({
                                    "Text": text,
                                    "Word": word
                                });
                                
                                // Cast as bytes
                                cast("parsed_strings", response_json.to_string().as_bytes());
                                (200, Some(response_json.to_string().into_bytes()))
                            } else {
                                (400, Some(Vec::<u8>::from("Missing or invalid string parameters")))
                            }
                        }
                        Err(_) => (400, Some(Vec::<u8>::from("Invalid JSON format")))
                    }
                } else {
                    (400, Some(Vec::<u8>::from("Body is not a valid string")))
                }
            } else {
                (404, Some(Vec::<u8>::from("Empty body")))
            }
        } else {
            (404, Some(Vec::<u8>::from("Invalid path")))
        };

        let res = EdgelessHTTPResponse {
            status: res_params.0,
            body: res_params.1,
            headers: std::collections::HashMap::<String, String>::new(),
        };

        CallRet::Reply(OwnedByteBuff::new_from_slice(edgeless_http::response_to_string(&res).as_bytes()))
    }

    fn handle_init(_payload: Option<&[u8]>, _serialized_state: Option<&[u8]>) {
        edgeless_function::init_logger();
        log::info!("http_read_strings: 'Init' called");
    }

    fn handle_stop() {
        log::info!("http_read_strings: 'Stop' called");
    }
}

edgeless_function::export!(HttpReadStringsFun);
