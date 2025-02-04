#!/bin/bash

lxterminal -e "bash -c 'cd edgeless/target/release && RUST_LOG=info ./edgeless_cli workflow start ../../../accelerometer_classification_workflow/workflow.json; exec bash'" &
