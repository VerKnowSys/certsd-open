use crate::*;

use hyperacme::{api::ApiProblem, create_p384_key, Directory, DirectoryUrl, Error};
use openssl::{
    ec::EcKey,
    pkey::{PKey, Private},
};
use std::{path::Path, time::Duration};
use tokio::{fs::File, io::AsyncWriteExt};


#[instrument]
pub async fn get_cert(domain: &str) -> Result<(), Error> {
    request_certificate(domain, false).await
}


#[instrument]
pub async fn get_cert_wildcard(domain: &str) -> Result<(), Error> {
    request_certificate(domain, true).await
}


#[instrument]
async fn request_certificate(domain: &str, wildcard: bool) -> Result<(), Error> {
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
        let account_str = tokio::fs::read_to_string(account_key_file_name).await?;
        dir.load_account(&account_str, contact.to_owned()).await?
    } else {
        info!("No account key present. Registering new account.");
        let new_account = dir.register_account(contact.to_owned()).await?;

        let mut account_file = File::create(account_key_file_name).await?;
        let pkey = new_account.acme_private_key_pem().await?;
        account_file.write_all(pkey.as_bytes()).await?;

        new_account
    };

    // Order a new TLS certificate for a domain.
    let mut ord_new = if wildcard {
        account.new_order(&format!("*.{domain}"), &[]).await?
    } else {
        account.new_order(domain, &[]).await?
    };

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

        if status == "invalid" {
            let api_problem = ApiProblem{
                detail: Some("Invalid status means that something went wrong with the LE API. Will try again later.".to_string()),
                subproblems: None,
                _type: String::from("ApiProblem")
            };
            return Err(Error::ApiProblem(api_problem));
        }

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

    let domain_dir = if wildcard {
        format!("wild_{domain}")
    } else {
        domain.to_string()
    };
    tokio::fs::create_dir_all(&domain_dir).await?;

    // Ownership is proven. Read a private key or create new for the certificate:
    let domain_key_filename = format!("{domain_dir}/domain.key");
    let domain_key = if !Path::new(&domain_key_filename).exists() {
        info!("Generating a new {domain_dir}/domain.key");
        let new_pkey = create_p384_key()?;

        let mut domain_key_file = File::create(format!("{domain_dir}/domain.key")).await?;
        domain_key_file
            .write_all(&new_pkey.private_key_to_pem_pkcs8()?)
            .await?;
        new_pkey
    } else {
        info!("Using previously known {domain_dir}/domain.key");
        let pkey_str = tokio::fs::read_to_string(domain_key_filename).await?;
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
    let mut cert_file = File::create(format!("{domain_dir}/chained.pem")).await?;
    cert_file.write_all(cert.certificate().as_bytes()).await?;

    info!("Done");
    Ok(())
}
