use crate::*;

use async_recursion::async_recursion;
use chrono::{prelude::*, Months};
use hyperacme::{
    api::ApiProblem,
    create_p384_key,
    order::{CsrOrder, NewOrder},
    Account, Certificate, Directory, DirectoryUrl, Error,
};
use openssl::{
    ec::EcKey,
    pkey::{PKey, Private},
};
use std::{os::unix::fs::PermissionsExt, path::Path};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    time::{sleep, Duration},
};


#[instrument(skip(config))]
pub async fn get_cert(config: &Config, domain: &str) -> Result<(), Error> {
    request_certificate(config, domain, false, 1).await
}


#[instrument(skip(config))]
pub async fn get_cert_wildcard(config: &Config, domain: &str) -> Result<(), Error> {
    request_certificate(config, domain, true, 1).await
}


#[instrument(skip(config, ord_new, domain))]
#[async_recursion]
async fn await_csr(
    config: &Config,
    mut ord_new: NewOrder,
    domain: &str,
    status: &str,
    order_attempts: usize,
) -> Result<CsrOrder, Error> {
    if let Some(ord_csr) = ord_new.confirm_validations().await {
        info!("Order confirmed.");
        return Ok(ord_csr);
    }

    if order_attempts > DEFAULT_MAX_ATTEMPTS {
        let api_problem = ApiProblem {
            detail: Some(format!(
                "Failed to order a Certificate within the {DEFAULT_MAX_ATTEMPTS} max confirmation attempts."
            )),
            subproblems: None,
            _type: String::from("ApiProblem"),
        };
        return Err(Error::ApiProblem(api_problem));
    }

    if status == "pending" {
        info!("Awaiting");
        sleep(Duration::from_millis(DEFAULT_ACME_POLL_PAUSE_MS)).await;
        ord_new.refresh().await?;
        return await_csr(config, ord_new, domain, status, order_attempts + 1).await;
    }

    // Get the possible authorizations
    let auths = ord_new.authorizations().await?;
    let auth = &auths[0]; // only a single wildcard per domain
    if auth.need_challenge().await {
        info!("Pending the domain registration");
        match auth.dns_challenge().await {
            Some(challenge) => {
                debug!("Deleting any previous DNS entries for domain: {domain}");
                delete_acme_dns_txt_entries(config, domain).await?;

                let proof_code = challenge.dns_proof().await?;
                match create_txt_record(config, domain, &proof_code).await {
                    Ok(_) => info!("DNS TXT record created"),
                    Err(err) => {
                        error!("Failed to create DNS TXT record. Error: {err:?}");
                    }
                }
                ord_new.refresh().await?;

                // The order at ACME will change status to either
                // confirm ownership of the domain, or fail due to the
                // not finding the proof. To see the change, we poll
                // the API with pause between.
                match challenge
                    .validate(Duration::from_millis(
                        DEFAULT_ACME_CHALLENGE_VALIDATION_PAUSE_MS,
                    ))
                    .await
                {
                    Ok(_) => {
                        info!("Challenge validated.");
                    }
                    Err(e) => {
                        debug!("Failed validation. Error {e:?}");
                    }
                }
                ord_new.refresh().await?;

                // delete the DNS TXT _acme entries
                delete_acme_dns_txt_entries(config, domain).await?;
            }
            None => {
                error!("Challenge is None!")
            }
        }
    } else {
        info!("Challenge not required.");
        ord_new.refresh().await?;
    }

    let status = &auth
        .api_auth()
        .await
        .to_owned()
        .status
        .unwrap_or("unknown".to_string());
    info!("Order status: {status:?}");

    if status == "invalid" {
        let api_problem = ApiProblem{
            detail: Some("Invalid status means that something went wrong with the LE API. Will try again later.".to_string()),
            subproblems: None,
            _type: String::from("ApiProblem")
        };
        return Err(Error::ApiProblem(api_problem));
    }

    // Call recursively until we get what we want
    await_csr(config, ord_new, domain, status, order_attempts + 1).await
}


#[instrument(skip(dir))]
async fn load_or_generate_new_account(
    contact: &Vec<String>,
    dir: &Directory,
) -> Result<Account, Error> {
    let account_key_file_name = "account.key";
    if Path::new(account_key_file_name).exists() {
        info!("Account key is present.");
        let account_str = tokio::fs::read_to_string(account_key_file_name).await?;
        dir.load_account(&account_str, contact.to_owned()).await
    } else {
        info!("No account key present. Registering new account.");
        let new_account = dir.register_account(contact.to_owned()).await?;

        let mut account_file = File::create(account_key_file_name).await?;
        let pkey = new_account.acme_private_key_pem().await?;
        account_file.write_all(pkey.as_bytes()).await?;
        set_private_key_permissions(account_key_file_name).await?;
        Ok(new_account)
    }
}


#[instrument]
async fn set_private_key_permissions(file_name: &str) -> Result<(), Error> {
    let mut perms = tokio::fs::metadata(&file_name).await?.permissions();
    perms.set_mode(0o600);
    tokio::fs::set_permissions(&file_name, perms).await?;
    Ok(())
}


#[instrument(skip(domain_dir))]
async fn load_or_generate_domain_key(
    domain_key_filename: &str,
    domain_dir: &str,
) -> Result<PKey<Private>, Error> {
    if !Path::new(&domain_key_filename).exists() {
        info!("Generating a new {domain_dir}/domain.key");
        let new_pkey = create_p384_key()?;
        let domain_key_file_name = &format!("{domain_dir}/domain.key");
        let mut domain_key_file = File::create(domain_key_file_name).await?;
        domain_key_file
            .write_all(&new_pkey.private_key_to_pem_pkcs8()?)
            .await?;
        set_private_key_permissions(domain_key_file_name).await?;
        Ok(new_pkey)
    } else {
        info!("Using previously known {domain_dir}/domain.key");
        let pkey_str = tokio::fs::read_to_string(domain_key_filename).await?;
        let ec_key: EcKey<Private> = EcKey::private_key_from_pem(pkey_str.as_bytes())?;
        Ok(PKey::from_ec_key(ec_key)?)
    }
}


#[instrument]
async fn read_certificate_expiry_date(
    chained_certifcate_file_name: &str,
    domain_key: &PKey<Private>,
) -> Result<DateTime<Utc>, Error> {
    let pkey_string = String::from_utf8(domain_key.private_key_to_pem_pkcs8()?)?;
    let current_cert_read = Certificate::parse(
        pkey_string,
        tokio::fs::read_to_string(chained_certifcate_file_name).await?,
    )?;
    current_cert_read.expiry()
}


// Order a new TLS certificate for a domain.
#[instrument(skip(account, domain, wildcard))]
async fn create_new_order(
    account: &Account,
    domain: &str,
    wildcard: bool,
) -> Result<NewOrder, Error> {
    if wildcard {
        account.new_order(&format!("*.{domain}"), &[]).await
    } else {
        account.new_order(domain, &[]).await
    }
}


#[async_recursion]
#[instrument(skip(config, domain, attempts))]
async fn request_certificate(
    config: &Config,
    domain: &str,
    wildcard: bool,
    attempts: usize,
) -> Result<(), Error> {
    if attempts > DEFAULT_MAX_ATTEMPTS {
        let err = format!(
            "Reached max retry attempts: {DEFAULT_MAX_ATTEMPTS}. Check the API credentials."
        );
        error!("{err}");
        return Err(hyperacme::Error::GeneralError(err));
    }
    let url = match config.acme_staging().await {
        true => DirectoryUrl::LetsEncryptStaging,
        _ => DirectoryUrl::LetsEncrypt,
    };
    info!("Using LE url: {url:?}");

    // Create a directory entrypoint.
    let dir = Directory::from_url(url).await?;

    let contacts = config
        .contacts_of(domain)
        .await
        .iter()
        .map(|contact| format!("mailto:{contact}"))
        .collect();

    // Generate a account.key if doesn't exist and register an account with your ACME provider:
    let account = load_or_generate_new_account(&contacts, &dir).await?;

    let domain_dir = if wildcard {
        format!("wild_{domain}")
    } else {
        domain.to_string()
    };
    tokio::fs::create_dir_all(&domain_dir).await?;

    // Read a domain private key or create new for the certificate:
    let domain_key_filename = format!("{domain_dir}/domain.key");
    let domain_key = load_or_generate_domain_key(&domain_key_filename, &domain_dir).await?;

    // check if the current Certificate is fresh enough
    let today = Local::now();
    let chained_certifcate_file = format!("{domain_dir}/chained.pem");
    if Path::new(&chained_certifcate_file).exists() {
        info!("Previous certificate exists: {chained_certifcate_file}.");
        let expiry_date =
            read_certificate_expiry_date(&chained_certifcate_file, &domain_key).await?;
        let today_plus_n_months = today + Months::new(DEFAULT_MAX_CERT_VALIDITY_IN_MONTHS);
        if today_plus_n_months < expiry_date {
            info!("Certificate expires at: {expiry_date}. No need to renew.");
            return Ok(());
        }
    }

    let ord_new = create_new_order(&account, domain, wildcard)
        .await
        .map(|ord_new| await_csr(config, ord_new, domain, "", 1))?
        .await;

    // If the ownership of the domain(s) have already been
    // authorized in a previous order, you might be able to
    // skip validation. The ACME API provider decides.
    let error_pause = sleep(Duration::from_millis(DEFAULT_ACME_INVALID_STATUS_PAUSE_MS));
    let ord_csr = match ord_new {
        Ok(order) => order,
        Err(Error::ApiProblem(api_problem)) => {
            let problem = api_problem.detail.unwrap_or_default();
            warn!("Waiting 30s to retry (attempts: {attempts}). API problem: {problem:?}");
            error_pause.await;
            return request_certificate(config, domain, wildcard, attempts + 1).await;
        }
        Err(err) => {
            warn!("Unhandled error: {err:?}. Waiting 30s to retry (attempts: {attempts})");
            error_pause.await;
            return request_certificate(config, domain, wildcard, attempts + 1).await;
        }
    };

    // Submit the CSR. This causes the ACME provider to enter a
    // state of "processing" that must be polled until the
    // certificate is either issued or rejected. Again we poll
    // for the status change.
    let ord_cert = ord_csr
        .finalize_pkey(
            domain_key.to_owned(),
            Duration::from_millis(DEFAULT_ACME_POLL_PAUSE_MS),
        )
        .await?;

    let today_date = today.date_naive();
    if Path::new(&chained_certifcate_file).exists() {
        info!(
            "Making a copy of the previous certificate to: {chained_certifcate_file}-{today_date}"
        );
        tokio::fs::copy(
            &chained_certifcate_file,
            format!("{}-{}", &chained_certifcate_file, today_date),
        )
        .await?;
    }

    // Now download the certificate. Also stores the cert persistently.
    let cert = ord_cert.download_cert().await?;
    let mut cert_file = File::create(chained_certifcate_file.to_owned()).await?;
    cert_file.write_all(cert.certificate().as_bytes()).await?;

    notify_success(config, domain, wildcard)
        .await
        .unwrap_or_default();

    info!("Ready");
    Ok(())
}
