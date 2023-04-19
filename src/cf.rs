use std::env;

use cloudflare::endpoints::dns::CreateDnsRecord;
use cloudflare::endpoints::dns::CreateDnsRecordParams;
use cloudflare::endpoints::dns::DnsContent;
use cloudflare::endpoints::dns::ListDnsRecordsParams;
use cloudflare::framework::async_api::Client;
use cloudflare::framework::{auth::Credentials, Environment, HttpApiClientConfig};


fn get_env_value_or_panic(env: &str) -> String {
    env::vars()
        .find(|(key, _)| key == env)
        .map(|(_, value)| value)
        .unwrap()
}

#[test]
fn test_get_env_value() {
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    assert!(zone_id.len() > 10);

    let api_token = &get_env_value_or_panic("CLOUDFLARE_API_TOKEN");
    assert!(api_token.len() > 10);
}


#[tokio::test]
async fn test_cf_api_dns_records_list() {
    // let domain = "centratests.com";
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");

    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )
    .unwrap_or_else(|_| panic!("Couldn't login to CF API"));

    let list_dns_txt_records = cloudflare::endpoints::dns::ListDnsRecords {
        zone_identifier: zone_id,
        params: ListDnsRecordsParams::default(),
    };
    let response = client.request_handle(&list_dns_txt_records).await.unwrap();

    let txt_record_ids = response
        .result
        .iter()
        .filter_map(|record| {
            match record.content.to_owned() {
                DnsContent::TXT {
                    content: _, /* the TXT entry is irrelevant to us, we only want to list TXT records… */
                } => {
                    // …that contain "_acme-challenge", since we also use other TXT records for MX-stuff
                    if record.name.contains("_acme-challenge") {
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

    // let response = list_dns_records.query().unwrap();
    println!("Res: {:#?}", txt_record_ids);
}


#[tokio::test]
async fn test_cf_api_delete() {
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");

    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )
    .unwrap_or_else(|_| panic!("Couldn't login to CF API"));

    let delete_dns_record = cloudflare::endpoints::dns::DeleteDnsRecord {
        zone_identifier: zone_id,
        identifier: "4ade8000201553984456ade29e54d62f",
    };
    let response = client.request_handle(&delete_dns_record).await.unwrap();

    println!("Res: {:#?}", response);
}


#[tokio::test]
async fn test_cf_api_create_txt_record() {
    let domain = "centratests.com";
    let zone_id = &get_env_value_or_panic("CLOUDFLARE_ZONE_ID");
    let client = Client::new(
        Credentials::UserAuthToken {
            token: get_env_value_or_panic("CLOUDFLARE_API_TOKEN"),
        },
        HttpApiClientConfig::default(),
        Environment::Production,
    )
    .unwrap_or_else(|_| panic!("Couldn't login to CF API"));

    let create_dns_txt_record = CreateDnsRecord {
        zone_identifier: zone_id,
        params: CreateDnsRecordParams {
            name: &format!("_acme-challenge.{domain}"),
            priority: None,
            proxied: Some(false),
            ttl: Some(60),
            content: DnsContent::TXT {
                content: "123456-challenge".to_string(),
            },
        },
    };
    let response = client.request_handle(&create_dns_txt_record).await.unwrap();

    println!("Res: {:#?}", response);
}
