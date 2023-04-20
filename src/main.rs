use certsd::*;

use hyperacme::create_p384_key;
use hyperacme::{Directory, DirectoryUrl, Error}; // Certificate,
use std::path::Path;
use std::time::Duration;
use std::{fs, io::Write};


// const DOMAINS: &[&str] = &["centra.fi"];


#[tokio::main]
async fn main() -> Result<(), Error> {
    let domain = "centratests.com";
    // let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");

    // Use DirectoryUrl::LetsEncrypStaging for dev/testing.
    let url = DirectoryUrl::LetsEncryptStaging;

    // Create a directory entrypoint.
    let dir = Directory::from_url(url).await?;

    // Your contact addresses, note the `mailto:`
    let contact = vec!["mailto:devteam@centra.com".to_string()];

    // Generate a account.key if doesn't exist and register an account with your ACME provider:
    let account = if Path::new("account.key").exists() {
        println!("Account key is present.");
        let account_str = fs::read_to_string("account.key")?;
        dir.load_account(&account_str, contact.to_owned()).await?
    } else {
        println!("No account key present. Registering new account.");
        let new_account = dir.register_account(contact.to_owned()).await?;

        let mut account_file = std::fs::File::create("account.key")?;
        let pkey = new_account.acme_private_key_pem().await?;
        account_file.write_all(pkey.as_bytes())?;

        new_account
    };

    // Order a new TLS certificate for a domain.
    let mut ord_new = account.new_order(&format!("*.{domain}"), &[]).await?;

    // If the ownership of the domain(s) have already been
    // authorized in a previous order, you might be able to
    // skip validation. The ACME API provider decides.
    let ord_csr = loop {
        // are we done?
        if let Some(ord_csr) = ord_new.confirm_validations().await {
            break ord_csr;
        }

        // Get the possible authorizations (for a single domain
        // this will only be one element).
        let auths = ord_new.authorizations().await?;

        let auth = &auths[0];
        let challenge = auth.dns_challenge().await.unwrap();
        let proof_code = challenge.dns_proof().await?;
        match create_txt_record(domain, &proof_code).await {
            Ok(_) => println!("DNS TXT record created"),
            Err(_e) => {} //println!("DNS record already defined!")
        }

        // The order at ACME will change status to either
        // confirm ownership of the domain, or fail due to the
        // not finding the proof. To see the change, we poll
        // the API with 5000 milliseconds wait between.
        match challenge.validate(Duration::from_millis(10000)).await {
            Ok(_) => println!("Challenge validated!"),
            Err(_e) => {}
        }

        // Update the state against the ACME API.
        ord_new.refresh().await?;
    };

    let dns_response = list_acme_txt_records().await;
    let the_list = dns_response.unwrap();
    for entry in the_list {
        delete_txt_record(&entry).await.unwrap();
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

    println!("Done");
    Ok(())
}
