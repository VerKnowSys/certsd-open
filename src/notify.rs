use crate::*;

use serde::Deserialize;
use slack_hook2::{AttachmentBuilder, PayloadBuilder, Slack};
use telegram_bot_api::{bot::*, methods::SendMessage, types::ChatId};


#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Default)]
pub enum NotifyWith {
    Slack {
        webhook: String,
    },
    Telegram {
        chat_id: String,
        token: String,
    },

    #[default]
    None,
}


impl NotifyWith {

    #[instrument(skip(self))]
    pub async fn notify(&self, message: &str) -> Result<()> {
        match self {
            NotifyWith::Slack {
                webhook,
            } => {
                info!("Slack notification configured");
                notify_success_slack(&webhook.to_owned(), message)
                    .await
            }
            NotifyWith::Telegram {
                chat_id,
                token,
            } => {
                info!("Telegram notification configured");
                notify_success_telegram(
                    chat_id,
                    token,
                    message,
                )
                .await
                .map_err(Into::into)
            }
            NotifyWith::None => {
                info!("No notification configured");
                Ok(())
            }
        }
    }
}


/// Sends success notification to Slack
#[instrument(skip(webhook))]
pub async fn notify_success_slack(webhook: &str, message: &str) -> Result<()> {
    let success_emoji = String::from(DEFAULT_SLACK_SUCCESS_ICON);
    notify_slack(webhook, message, &success_emoji, false)
        .await
}


/// Sends success notification to Telegram
#[instrument(skip(token))]
pub async fn notify_success_telegram(
    chat_id: &str,
    token: &str,
    message: &str,
) -> Result<()> {
    let success_emoji = String::from(DEFAULT_SLACK_SUCCESS_ICON);
    notify_telegram(chat_id, token, message, &success_emoji, false)
        .await
}


/// Send notification to Slack with retry on failure
#[instrument(skip(config, domain, wildcard))]
pub async fn notify_success(
    config: &Config,
    domain: &str,
    wildcard: bool,
) -> Result<()> {
    let message = if wildcard {
        format!("Certificate renewal succeeded for the domain: *.{domain}.")
    } else {
        format!("Certificate renewal succeeded for the domain: {domain}.")
    };
    for notification_type in config.notifications.iter() {
        notification_type.notify(&message).await.map_err(|e| error!("{e}")).unwrap_or_default();
    }
    Ok(())
}


#[instrument(skip(token))]
pub async fn notify_telegram(
    chat_id: &str,
    token: &str,
    message: &str,
    icon: &str,
    fail: bool,
) -> Result<()> {
    if chat_id.is_empty() {
        warn!("Telegram Chat ID is undefined. Notifications will not be sent.");
        return Ok(());
    }
    if token.is_empty() {
        warn!("Telegram Token is undefined. Notifications will not be sent.");
        return Ok(());
    }
    let api = match BotApi::new(String::from(token), None).await {
        Ok(api) => {
            api
        },
        Err(err) => return Err(anyhow!("Telegram BotApi failed: {err:?}"))
    };
    let message = SendMessage::new(ChatId::StringType(String::from(chat_id)), String::from(message));
    match api.send_message(message).await {
        Ok(_sent) => Ok(()),
        Err(err) => Err(anyhow!("Telegram BotApi couldn't connect to chat: {chat_id}. Error: {err:?}"))
    }
}


/// Sends generic notification over Slack
#[instrument(skip(webhook))]
async fn notify_slack(webhook: &str, message: &str, icon: &str, fail: bool) -> Result<()> {
    if webhook.is_empty() {
        warn!("Slack Webhook is undefined. Notifications will not be sent.");
        return Ok(());
    }
    let slack = Slack::new(webhook)?;
    let payload = PayloadBuilder::new()
        .username(DEFAULT_SLACK_NAME)
        .icon_emoji(icon)
        .attachments(vec![
            if fail {
                AttachmentBuilder::new(message)
                    .color(DEFAULT_SLACK_FAILURE_COLOR)
                    .build()
                    .unwrap_or_default()
            } else {
                AttachmentBuilder::new(message)
                    .color(DEFAULT_SLACK_SUCCESS_COLOR)
                    .build()
                    .unwrap_or_default()
            },
        ])
        .build()?;

    slack.send(&payload).await.map_err(Into::into)
}


#[tokio::test]
async fn test_send_message() -> Result<()> {

    use super::*;
    use std::path::Path;

    if Path::new("certsd.conf").exists() {
        initialize_logger();
        info!("Starting new message test for Telegram since certsd.conf is present in the CWD");

        let config = Config::from("certsd.conf").await?;
        assert!(config.acme_staging().await);
        let telegram_kind = config.notifications.iter().find(|elem| {
            matches!(elem, NotifyWith::Telegram {
                    ..
                })
        });
        if let Some(telegram) = telegram_kind {
            let NotifyWith::Telegram {
                chat_id,
                token,
            } = telegram else { todo!() };
            notify_telegram(
                chat_id,
                token,
                "Testing message",
                "icon",
                false,
            )
            .await
            .unwrap();
        }
    }
    Ok(())
}
