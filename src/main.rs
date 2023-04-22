use certsd::*;
use hyperacme::Error;
use tracing_subscriber::{
    fmt::{
        format::{Compact, DefaultFields, Format},
        Layer, *,
    },
    layer::Layered,
    reload::*,
    EnvFilter, Registry,
};


type TracingEnvFilterHandle =
    Handle<EnvFilter, Layered<Layer<Registry, DefaultFields, Format<Compact>>, Registry>>;


#[instrument]
fn initialize_logger() -> TracingEnvFilterHandle {
    let env_log_filter = match EnvFilter::try_from_env("LOG") {
        Ok(env_value_from_env) => env_value_from_env,
        Err(_) => EnvFilter::from("info"),
    };
    let fmt = fmt()
        .compact()
        .with_target(true)
        .with_line_number(false)
        .with_file(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_ansi(true)
        .with_env_filter(env_log_filter)
        .with_filter_reloading();

    let handle = fmt.reload_handle();
    fmt.init();
    handle
}


#[instrument]
#[tokio::main] // (flavor = "current_thread")
async fn main() -> Result<(), Error> {
    initialize_logger();
    dotenv::dotenv().ok();

    let domains = get_env_value_or_panic("DOMAINS")
        .split_ascii_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let wildcard = matches!(
        get_env_value_or_panic("DOMAINS_WILDCARD").as_str(),
        "YES" | "yes" | "on" | "ON" | "1"
    );

    info!("Processing domains: {domains:?} (wildcard: {wildcard})");
    for domain in domains {
        if wildcard {
            get_cert_wildcard(&domain).await?
        } else {
            get_cert(&domain).await?
        }
    }

    Ok(())
}
