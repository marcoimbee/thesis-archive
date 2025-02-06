use edgeless_orc::proxy::Proxy;
use redis::Commands;
use std::fs::File;
use std::io::Read;
use serde_json::Value;
use std::process::Command;

/*
    TODO: (maybe?) rebalance only if the latency changed of some quantity from the last time
*/

#[derive(Debug)]
struct NodeDesc {
    function_instances: Vec<edgeless_api::function_instance::ComponentId>,
    capabilities: edgeless_api::node_registration::NodeCapabilities,    
    resource_providers: std::collections::HashSet<String>,
    node_to_orc_latency: f64,
}

#[derive(Debug)]
struct InstanceDesc {
    runtime: String,            // Python or Rust

    // Deployment requirements for functions, as specified by annotations
    // Among others:
    //      -> node_id_match_any: Vec<uuid::Uuid>
    deployment_requirements: edgeless_orc::deployment_requirements::DeploymentRequirements,
    relocatable: bool,          // Will be true if node_id_match_any is empty
}

pub struct NetworkAwareOrchestrator {
    proxy: edgeless_orc::proxy_redis::ProxyRedis,
    redis_url: String,
    nodes: std::collections::HashMap<edgeless_api::function_instance::NodeId, NodeDesc>,
    instances:std::collections::HashMap<edgeless_api::function_instance::ComponentId, InstanceDesc>,
    latency_threshold: f64,
    relocation_mode: String,
}

impl NetworkAwareOrchestrator {
    pub fn new(redis_url: &str, latency_threshold: f64, relocation_mode: &str) -> anyhow::Result<Self> {
        let proxy = match edgeless_orc::proxy_redis::ProxyRedis::new(redis_url, false, None) {
            Ok(proxy) => proxy,
            Err(err) => anyhow::bail!("could not connect to Redis at {}: {}", redis_url, err),
        };

        Ok(Self {
            proxy,
            redis_url: redis_url.to_string(),
            nodes: std::collections::HashMap::new(),
            instances: std::collections::HashMap::new(),
            latency_threshold,
            relocation_mode: relocation_mode.to_string(),
        })
    }

    pub fn move_to_rpi(&mut self, rpi_node_id: &str) -> (bool, u32) {
        self.get_function_instances();
        self.update_node_desc();

        let mut counter = 0;
        for (node_id, node_desc) in &self.nodes {
            for lid in &node_desc.function_instances {
                let instance_desc = self
                    .instances
                    .get(lid)
                    .expect("Function instance disappeared");

                let status = Command::new("../edgeless/target/debug/proxy_cli")
                    .arg("intent")
                    .arg("migrate")
                    .arg(lid.to_string())
                    .arg(rpi_node_id)
                    .status()
                    .expect("Failed to execute proxy_cli command");

                counter += 1;

                if status.success() {
                    println!("[INFO] Successfully moved");
                } else {
                    eprintln!("[ERROR] Command failed with status: {}", status);
                    return (false, counter);
                }
            }
        }

        (true, counter)
    }

    pub fn monitor_cluster(&mut self) {
        // Init NAO structure
        self.get_function_instances();
        println!("----------------------------------------------------------------------------------");

        // Fetch the status of nodes
        self.update_node_desc();
        println!("----------------------------------------------------------------------------------");

        for (node_id, node_desc) in &self.nodes {
            for lid in &node_desc.function_instances {
                let instance_desc = self
                    .instances
                    .get(lid)
                    .expect("Function instance disappeared");

                if instance_desc.relocatable {
                    println!("[INFO] {} (spawned on: {}) is relocatable", lid, node_id);
                }
            }
        }
    }

    pub fn rebalance(&mut self) -> (bool, usize) {
        // if !self.proxy.updated(edgeless_orc::proxy::Category::NodeCapabilities)
        //     && !self.proxy.updated(edgeless_orc::proxy::Category::ResourceProviders)
        //     && !self.proxy.updated(edgeless_orc::proxy::Category::ActiveInstances)
        // {
        //     println!("[INFO] Nothing has changed since the last iteration. Finishing...");
        //     return (false, 0);
        // }

        // Init NAO structure
        self.get_function_instances();
        println!("----------------------------------------------------------------------------------");

        // Fetch the status of nodes
        self.update_node_desc();
        println!("----------------------------------------------------------------------------------");

        // Try to rebalance things
        (true, self.migrate())
    }

    fn get_function_instances(&mut self) {
        self.instances.clear();

        let mut instances = self.proxy.fetch_function_instance_requests();
        for (lid, req) in &mut instances {
            let runtime = req.code.function_class_type.clone();
            let deployment_requirements = edgeless_orc::deployment_requirements::DeploymentRequirements::from_annotations(
                &req.annotations,
            );

            let relocatable = deployment_requirements.node_id_match_any.is_empty();

            let instance_desc = InstanceDesc {
                runtime,
                deployment_requirements,
                relocatable,
            };

            self.instances.insert(
                *lid,
                instance_desc,
            );
        }
    }

    fn update_node_desc(&mut self) {
        // Create node descriptors, with capabilities
        self.nodes.clear();

        for (node_id, capabilities) in self.proxy.fetch_node_capabilities() {
            self.nodes.insert(
                node_id,
                NodeDesc {
                    function_instances: vec![],
                    capabilities,
                    resource_providers: std::collections::HashSet::new(),
                    node_to_orc_latency: 0.0,
                },
            );

            println!("[INFO] Fetched capabilities for node {}", node_id);
        }

        // Add function instances, only keep those that do not have any
        // specific deployment requirements (node_id_match_any annotation set)
        // Those will be the relocatable functions
        let mut instances = self.proxy.fetch_nodes_to_instances();
        if instances.is_empty() {
            println!("[INFO] No function instances found");
        } else {
            for (node_id, instances) in &mut instances {
                let node_function_instances = &mut self
                    .nodes
                    .get_mut(node_id)
                    .expect("cannot find node")
                    .function_instances;
                
                let func_instances = instances.len();
                println!("[INFO] Fetched {} function instances for node {}", func_instances, node_id);
                for instance in instances {
                    if let edgeless_orc::proxy::Instance::Function(lid) = instance {
                        println!("\t-> LID: {}", lid.to_string());

                        node_function_instances.push(*lid);
                    }
                }
            }
        }

        // Add resource providers
        let providers = self.proxy.fetch_resource_providers();
        for (provider_id, resource_provider) in providers {
            let node_resource_providers = &mut self
                .nodes
                .get_mut(&resource_provider.node_id)
                .expect("cannot find node")
                .resource_providers;
            node_resource_providers.insert(provider_id);
        }

        // Write node-to-orchestrator latencies
        let client = redis::Client::open(self.redis_url.clone()).expect("[ERROR] Failed to create Redis client");
        let mut conn = client.get_connection().expect("[ERROR] Failed to get Redis connection");
        for (node_id, node_desc) in &mut self.nodes {
            let redis_key = format!("latency:{}:orc", node_id);

            match conn.get::<_, Option<f64>>(&redis_key) {
                Ok(Some(latency_value)) => {
                    node_desc.node_to_orc_latency = latency_value;
                    println!("[INFO] Updated node {} with latency {}", node_id, latency_value);
                }
                Ok(None) => {
                    println!("[WARN] Key {} not found", redis_key);
                }
                Err(err) => {
                    println!("[ERROR] failed to fetch key {}", redis_key);
                }
            }
        }
    }

    fn migrate(&mut self) -> usize {
        // Determine the node which is closest to the ORC
        let closest_node_id = self.nodes
            .iter()
            .min_by(|(_, desc_1), (_, desc_2)| desc_1.node_to_orc_latency.partial_cmp(&desc_2.node_to_orc_latency).unwrap())
            .map(|(node_id, _)| node_id.clone())
            .unwrap();

        println!("[INFO] Closest node to the E-ORC: {}", closest_node_id);

        let mut migrations = Vec::new();        // Migrations vector

        println!("----------------------------------------------------------------------------------");
        // Determine migrations to be performed
        for (node_id, node_desc) in &self.nodes {
            for lid in &node_desc.function_instances {
                let instance_desc = self
                    .instances
                    .get(lid)
                    .expect("Function instance disappeared");

                if instance_desc.relocatable {
                    println!("[INFO] {} (spawned on: {}) is relocatable", lid, node_id);

                    if !(node_id == &closest_node_id) {
                        println!("\t-> Relocating {} from {} to {}...", lid, node_id, closest_node_id);

                        migrations.push(edgeless_orc::deploy_intent::DeployIntent::Migrate(
                            *lid,
                            vec![closest_node_id.clone()],
                        ));
                    }
                }
            }
        }

        if !migrations.is_empty() {
            self.proxy.add_deploy_intents(migrations.clone());
        }

        migrations.len()
    }
}
