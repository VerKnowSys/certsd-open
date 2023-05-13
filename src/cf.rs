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


#[instrument(skip(config))]
pub async fn delete_acme_dns_txt_entries(
    config: &Config,
    domain: &str,
) -> Result<(), hyperacme::Error> {
    let dns_response = list_acme_txt_records(config, domain).await;
    match dns_response {
        Ok(the_list) => {
            for entry in the_list {
                match delete_txt_record(config, domain, &entry).await {
                    Ok(_) => info!("DNS TXT record destroyed"),
                    Err(_) => error!("No DNS record to destroy"),
                }
            }
        }
        Err(e) => error!("Err: {e}"),
    }
    Ok(())
}


#[instrument(skip(config))]
pub async fn list_acme_txt_records(config: &Config, domain: &str) -> Result<Vec<String>> {
    let zone_id = config.zone_id_of(domain).await;
    let client = Client::new(
        Credentials::UserAuthToken {
            token: config.api_token_of(domain).await,
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let list_dns_txt_records = ListDnsRecords {
        zone_identifier: &zone_id,
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


#[instrument(skip(config))]
pub async fn delete_txt_record(
    config: &Config,
    domain: &str,
    id: &str,
) -> Result<ApiSuccess<DeleteDnsRecordResponse>> {
    let zone_id = config.zone_id_of(domain).await;
    let client = Client::new(
        Credentials::UserAuthToken {
            token: config.api_token_of(domain).await,
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let delete_dns_record = DeleteDnsRecord {
        zone_identifier: &zone_id,
        identifier: id,
    };
    client
        .request_handle(&delete_dns_record)
        .await
        .map_err(|e| e.into())
}


#[instrument(skip(config))]
pub async fn create_txt_record(
    config: &Config,
    domain: &str,
    content: &str,
) -> Result<ApiSuccess<DnsRecord>> {
    let zone_id = config.zone_id_of(domain).await;
    let client = Client::new(
        Credentials::UserAuthToken {
            token: config.api_token_of(domain).await,
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;
    let create_dns_txt_record = CreateDnsRecord {
        zone_identifier: &zone_id,
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
