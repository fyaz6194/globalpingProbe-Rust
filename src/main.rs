mod command;
mod config;
mod probe;
mod status;
mod util;

use probe::{client::ClientConfig, uuid::ProbeUuid};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    util::logger::init();

    let uuid_path = std::env::var("GP_UUID_PATH")
        .unwrap_or_else(|_| probe::uuid::resolve_uuid_path());
    let uuid = ProbeUuid::load_or_create(&uuid_path);
    let cfg = ClientConfig {
        api_host: std::env::var("GP_API_HOST")
            .unwrap_or_else(|_| "https://api.globalping.io".into()),
        uuid: uuid.id,
        ping_target: std::env::var("GP_PING_TARGET")
            .unwrap_or_else(|_| "api.globalping.io".into()),
        adoption_token: std::env::var("GP_ADOPTION_TOKEN").ok(),
    };

    probe::client::run(cfg).await
}
