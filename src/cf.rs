use crate::*;

use cloudflare::{
    endpoints::dns::{
        CreateDnsRecord, CreateDnsRecordParams, DeleteDnsRecord, DeleteDnsRecordResponse,
        DnsContent, DnsRecord, ListDnsRecords, ListDnsRecordsParams,
    },
    framework::{
        async_api::Client, auth::Credentials, response::ApiSuccess, Environment,
        HttpApiClientConfig,
    },
};


pub fn get_env_value_or_panic(env: &str) -> String {
    std::env::vars()
        .find(|(key, _)| key == env)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("Required env value: '{env}' is empty."))
}


#[test]
fn test_get_env_value() {
    dotenv::dotenv().ok();

    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    assert!(zone_id.len() > 10);

    let api_token = &get_env_value_or_panic("CLOUDFLARE_API_TOKEN");
    assert!(api_token.len() > 10);
}


pub async fn delete_acme_dns_txt_entries(domain: &str) -> Result<(), hyperacme::Error> {
    let dns_response = list_acme_txt_records(domain).await;
    match dns_response {
        Ok(the_list) => {
            for entry in the_list {
                match delete_txt_record(&entry).await {
                    Ok(_) => info!("DNS TXT record destroyed"),
                    Err(_) => info!("No DNS record to destroy"),
                }
            }
        }
        Err(e) => error!("Err: {e}"),
    }
    Ok(())
}


pub async fn list_acme_txt_records(domain: &str) -> Result<Vec<String>, anyhow::Error> {
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let list_dns_txt_records = ListDnsRecords {
        zone_identifier: zone_id,
        params: ListDnsRecordsParams::default(),
    };
    let response = client.request_handle(&list_dns_txt_records).await?;

    let txt_record_ids = response
        .result
        .iter()
        .filter_map(|record| {
            match record.content.to_owned() {
                DnsContent::TXT {
                    content: _, /* the TXT entry is irrelevant to us, we only want to list TXT records… */
                } => {
                    // …that contain "_acme-challenge", since we also use other TXT records for MX-stuff
                    if record.name.contains("_acme-challenge") && record.name.contains(domain)
                    {
                        Some(record.id.to_owned())
                    } else {
                        None
                    }
                }
                _ => {
                    // ignore all other record types
                    None
                }
            }
        })
        .collect::<Vec<String>>();
    // no need to handle errors here, as the empty list of records is still a valid response
    Ok(txt_record_ids)
}


#[tokio::test]
async fn test_list_acme_txt_records() {
    dotenv::dotenv().ok();

    let domain = "centratests.com";
    let response = list_acme_txt_records(domain).await;
    println!("acme txt records: {response:#?}");
    assert!(response.is_ok());
}


pub async fn delete_txt_record(
    id: &str,
) -> Result<ApiSuccess<DeleteDnsRecordResponse>, anyhow::Error> {
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let delete_dns_record = DeleteDnsRecord {
        zone_identifier: zone_id,
        identifier: id,
    };
    client
        .request_handle(&delete_dns_record)
        .await
        .map_err(|e| e.into())
}


pub async fn create_txt_record(
    domain: &str,
    content: &str,
) -> Result<ApiSuccess<DnsRecord>, anyhow::Error> {
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let create_dns_txt_record = CreateDnsRecord {
        zone_identifier: zone_id,
        params: CreateDnsRecordParams {
            name: &format!("_acme-challenge.{domain}."),
            priority: None,
            proxied: Some(false),
            ttl: Some(60),
            content: DnsContent::TXT {
                content: content.to_string(),
            },
        },
    };

    client
        .request_handle(&create_dns_txt_record)
        .await
        .map_err(|e| e.into())
}


#[tokio::test]
async fn test_create_txt_record() {
    dotenv::dotenv().ok();

    let domain = "centratests.com";
    let response = create_txt_record(domain, "jakietakie").await;
    assert!(response.is_ok());
}


#[tokio::test]
async fn test_create_list_and_destroy_all_acme_txt_records() {
    dotenv::dotenv().ok();

    // create a record:
    let domain = "centratests.com";
    let response = create_txt_record(domain, "jakietakie123-oasdofs").await;
    assert!(response.is_ok());

    let double_entry_response = create_txt_record(domain, "jakietakie123-oasdofs").await;
    assert!(double_entry_response.is_err());

    // list all TXT records
    let response = list_acme_txt_records(domain).await;
    println!("acme txt records: {response:#?}");
    assert!(response.is_ok());
    let the_list = response.unwrap();
    assert!(!the_list.is_empty());

    // delete all acme TXT records
    for entry in the_list {
        let response = delete_txt_record(&entry).await;
        assert!(response.is_ok())
    }

    // confirm that no more acme TXT records are defined:
    let response = list_acme_txt_records(domain).await;
    assert!(response.is_ok());
    let the_list = response.unwrap();
    assert!(the_list.is_empty());
}
