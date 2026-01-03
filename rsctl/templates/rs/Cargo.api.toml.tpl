[package]
name = "{{ serviceName }}"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "{{ serviceName }}"
path = "{{ serviceName }}.rs"

[dependencies]
anyhow = "1"
http = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
hyper = "0.14"
halo_micro = { package = "halo-micro", path = "../../../halo", features = {{ restFeatures }} }
{{ validatorDep }}
{{ authDep }}

[workspace]



