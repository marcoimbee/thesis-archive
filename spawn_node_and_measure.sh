#!/bin/bash

lxterminal -e "bash -c 'cd edgeless/target/release && RUST_LOG=info ./edgeless_node_d; exec bash'" &
lxterminal -e "bash -c 'cd node_to_orc_latency_measurement && cargo run; exec bash'" &
