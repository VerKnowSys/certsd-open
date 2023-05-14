use crate::*;
use ron::de::*;

use serde::Deserialize;
use std::path::Path;
use tokio::fs::read_to_string;


#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    pub acme_staging: bool,
    pub notifications: Vec<NotifyWith>,
    pub accounts: Vec<CloudFlareAccount>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CloudFlareAccount {
    pub cloudflare_api_token: String,
    pub cloudflare_zone_id: String,
    pub domain: String,
    pub contacts: Vec<String>,
}


const CONFIG_PATHS: [&str; 5] = [
    "/etc/certsd/certsd.conf",
    "/Services/Certsd/service.conf",
    "/Projects/certsd/certsd.conf",
    "/Volumes/Projects/certsd/certsd.conf",
    "certsd.conf",
];


impl Config {
    #[instrument]
    pub async fn config_file() -> String {
        CONFIG_PATHS
            .iter()
            .filter(|file| Path::new(file).exists())
            .take(1)
            .cloned()
            .collect()
    }


    #[instrument]
    pub async fn config_dir() -> String {
        let config_file = Self::config_file().await;
        match Path::new(&config_file).parent() {
            Some(path) if path.to_string_lossy().is_empty() => String::from("."),
            Some(path) => path.to_string_lossy().to_string(),
            None => String::from("."),
        }
    }


    #[instrument]
    pub async fn config_data_dir() -> Result<String> {
        let data_dir = format!("{}/certs", Config::config_dir().await);
        tokio::fs::create_dir_all(data_dir.to_owned()).await?;
        Ok(data_dir)
    }


    #[instrument]
    pub async fn load() -> Result<Config, SpannedError> {
        let config_file = Self::config_file().await;
        info!("Loading the configuration from: {config_file}");
        from_str::<Config>(&read_to_string(config_file).await?)
    }


    #[instrument]
    pub async fn from(config_file: &str) -> Result<Config, SpannedError> {
        info!("Loading the configuration from: {config_file}");
        from_str::<Config>(&read_to_string(config_file).await?)
    }


    #[instrument]
    pub async fn domains(&self) -> Vec<String> {
        self.accounts
            .iter()
            .cloned()
            .map(|accts| accts.domain)
            .collect()
    }


    #[instrument]
    pub async fn contacts_of(&self, domain: &str) -> Vec<String> {
        self.accounts
            .iter()
            .cloned()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.contacts)
            .unwrap_or_default()
    }


    #[instrument]
    pub async fn api_token_of(&self, domain: &str) -> String {
        self.accounts
            .iter()
            .cloned()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.cloudflare_api_token)
            .unwrap_or_default()
    }


    #[instrument]
    pub async fn zone_id_of(&self, domain: &str) -> String {
        self.accounts
            .iter()
            .cloned()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.cloudflare_zone_id)
            .unwrap_or_default()
    }


    #[instrument]
    pub async fn notifications(&self) -> Vec<NotifyWith> {
        self.notifications.to_owned()
    }


    #[instrument]
    pub async fn acme_staging(&self) -> bool {
        self.acme_staging
    }
}


#[tokio::test]
async fn test_config_load() -> Result<()> {
    let config = Config::from("certsd.test.conf").await?;
    assert!(config.acme_staging().await);
    assert_eq!(
        config.domains().await,
        vec!["the-domain.com", "the-second-domain.com"]
    );

    let domain = "the-domain.com";
    assert_eq!(
        config.contacts_of(domain).await,
        ["me@example.com", "someone@example.com"]
    );
    let zone_id = config.zone_id_of(domain).await;
    assert_eq!(&zone_id, "the-zone-id");

    let domain = "the-second-domain.com";
    assert_eq!(config.contacts_of(domain).await, ["another.me@example.com"]);
    let zone_id = config.zone_id_of(domain).await;
    assert_eq!(&zone_id, "the-second-zone-id");
    let api_token = config.api_token_of(domain).await;
    assert_eq!(&api_token, "the-second-api-token");

    config.notifications.iter().for_each(|elem| {
        match elem {
            NotifyWith::Slack {
                webhook,
            } => {
                assert_eq!(
                    "https://hooks.slack.com/services/111111111/33333333333/44444444444444444",
                    webhook
                );
            }

            NotifyWith::Telegram {
                chat_id,
                token,
            } => {
                assert_eq!("@Public_Channel", chat_id);
                assert_eq!("1111111111111111111111111111111", token);
            }

            NotifyWith::None => {
                panic!("Shouldn't have None!");
            }
        }
    });

    Ok(())
}
