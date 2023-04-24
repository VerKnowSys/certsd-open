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
#[tokio::main(flavor = "current_thread")] //(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Error> {
    initialize_logger();

    // Config validation
    let config = match Config::load().await {
        Ok(config) => {
            debug!("The configuration is: {config:#?}");
            config
        }
        Err(e) => {
            panic!("Unable to load the config: {e:#?}")
        }
    };

    let domains = config.domains().await;
    info!("Processing domains: {domains:?}");
    for domain in domains {
        get_cert_wildcard(&config, &domain)
            .await
            .and(get_cert(&config, &domain).await)?;
    }

    Ok(())
}
