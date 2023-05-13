use certsd::*;
use hyperacme::Error;


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
    let version = env!("CARGO_PKG_VERSION");
    info!(
        "{DEFAULT_SLACK_NAME} v{version} will generate certificates for domains: {domains:?}"
    );
    for domain in domains {
        get_cert_wildcard(&config, &domain)
            .await
            .and(get_cert(&config, &domain).await)?;
    }

    Ok(())
}
