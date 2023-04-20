use certsd::*;

use hyperacme::{create_p384_key, Directory, DirectoryUrl, Error};
use openssl::{
    ec::EcKey,
    pkey::{PKey, Private},
};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::Duration,
};


// const DOMAINS: &[&str] = &["centra.fi"];

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
#[tokio::main]
async fn main() -> Result<(), Error> {
    initialize_logger();

    let domain = "centratests.com";

    // Use DirectoryUrl::LetsEncrypStaging for dev/testing.
    let url = match get_env_value_or_panic("LE_STAGING").as_ref() {
        "YES" | "yes" | "1" | "true" => DirectoryUrl::LetsEncryptStaging,
        _ => DirectoryUrl::LetsEncrypt,
    };
    info!("Using LE url: {url:?}");

    // Create a directory entrypoint.
    let dir = Directory::from_url(url).await?;

    // Your contact addresses, note the `mailto:`
    let contact = vec!["mailto:devteam@centra.com".to_string()];

    // Generate a account.key if doesn't exist and register an account with your ACME provider:
    let account_key_file_name = "account.key";
    let account = if Path::new(account_key_file_name).exists() {
        info!("Account key is present.");
        let account_str = fs::read_to_string(account_key_file_name)?;
        dir.load_account(&account_str, contact.to_owned()).await?
    } else {
        info!("No account key present. Registering new account.");
        let new_account = dir.register_account(contact.to_owned()).await?;

        let mut account_file = File::create(account_key_file_name)?;
        let pkey = new_account.acme_private_key_pem().await?;
        account_file.write_all(pkey.as_bytes())?;

        new_account
    };

    // Order a new TLS certificate for a domain.
    let mut ord_new = account
        .new_order(&format!("*.{domain}"), &[])
        // .new_order(domain, &[])
        .await?;

    // If the ownership of the domain(s) have already been
    // authorized in a previous order, you might be able to
    // skip validation. The ACME API provider decides.
    let ord_csr = loop {
        // are we done?
        if let Some(ord_csr) = ord_new.confirm_validations().await {
            info!("Order confirmed.");
            break ord_csr;
        }

        // Get the possible authorizations
        let auths = ord_new.authorizations().await?;
        let auth = &auths[0]; // only a single wildcard per domain
        if auth.need_challenge().await {
            info!("Pending {}", auth.domain_name().await);
            match auth.dns_challenge().await {
                Some(challenge) => {
                    let proof_code = challenge.dns_proof().await?;
                    // info!("Proof code: {proof_code}");
                    match create_txt_record(domain, &proof_code).await {
                        Ok(_) => info!("DNS TXT record created"),
                        Err(_e) => {} //info!("DNS record already defined!")
                    }

                    // The order at ACME will change status to either
                    // confirm ownership of the domain, or fail due to the
                    // not finding the proof. To see the change, we poll
                    // the API with pause between.
                    match challenge.validate(Duration::from_millis(5000)).await {
                        Ok(_) => {
                            info!("Challenge validated.")
                        }
                        Err(_e) => {
                            // info!("Failed validation. Error {e:#?}")
                        }
                    }
                }
                None => {
                    error!("DNS challenge is none.")
                }
            }
        }
        // ord_new.refresh().await?;
        let status = &auth
            .api_auth()
            .await
            .to_owned()
            .status
            .unwrap_or("unknown".to_string());
        info!("Status {status:?}");

        // Update the state against the ACME API.
        ord_new.refresh().await?;
    };

    let dns_response = list_acme_txt_records(domain).await;
    match dns_response {
        Ok(the_list) => {
            for entry in the_list {
                match delete_txt_record(&entry).await {
                    Ok(_) => info!("DNS TXT record destroyed"),
                    Err(_) => debug!("No DNS record to destroy"),
                }
            }
        }
        Err(e) => error!("Err: {e}"),
    }

    fs::create_dir_all(domain)?;

    // Ownership is proven. Read a private key or create new for the certificate:
    let domain_key_filename = format!("{domain}/domain.key");
    let domain_key = if !Path::new(&domain_key_filename).exists() {
        info!("Generating a new {domain}/domain.key");
        let new_pkey = create_p384_key()?;

        let mut domain_key_file = File::create(format!("{domain}/domain.key"))?;
        domain_key_file.write_all(&new_pkey.private_key_to_pem_pkcs8()?)?;
        new_pkey
    } else {
        info!("Using previously known {domain}/domain.key");
        let pkey_str = fs::read_to_string(domain_key_filename)?;
        let ec_key: EcKey<Private> = EcKey::private_key_from_pem(pkey_str.as_bytes())?;
        PKey::from_ec_key(ec_key)?
    };

    // Submit the CSR. This causes the ACME provider to enter a
    // state of "processing" that must be polled until the
    // certificate is either issued or rejected. Again we poll
    // for the status change.
    let ord_cert = ord_csr
        .finalize_pkey(domain_key.to_owned(), Duration::from_millis(5000))
        .await?;

    // Now download the certificate. Also stores the cert persistently.
    let cert = ord_cert.download_cert().await?;
    let mut cert_file = File::create(format!("{domain}/fullchain.cer"))?;
    cert_file.write_all(cert.certificate().as_bytes())?;

    info!("Done");
    Ok(())
}
