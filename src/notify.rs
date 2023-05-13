use crate::*;

use slack_hook2::{AttachmentBuilder, PayloadBuilder, Slack, SlackError};
use tokio::time::Duration;
use try_again::{retry_async, Delay, Retry, TokioSleep};


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
    pub async fn notify(&self, message: &str) -> Result<(), anyhow::Error> {
        match self {
            NotifyWith::Slack {
                webhook,
            } => {
                info!("Slack notification");
                notify_success_slack(&webhook.to_owned(), message)
                    .await
                    .map_err(Into::into)
            }
            NotifyWith::Telegram {
                chat_id,
                token,
            } => {
                info!("Telegram notification");
                notify_success_telegram(
                    ChatId::StringType(chat_id.to_owned()),
                    &token.to_owned(),
                    message,
                )
                .await
                .map_err(Into::into)
            }
            NotifyWith::None => {
                info!("No notification");
                Ok(())
            }
        }
    }
}


/// Sends success notification to Slack
#[instrument]
pub async fn notify_success(webhook: &str, message: &str) -> Result<(), SlackError> {
    let success_emoji = String::from(DEFAULT_SLACK_SUCCESS_ICON);
    notify(webhook, message, &success_emoji, false).await
}


/// Sends failure notification to Slack
#[instrument]
pub async fn notify_failure(webhook: &str, message: &str) -> Result<(), SlackError> {
    let failure_emoji = String::from(DEFAULT_SLACK_FAILURE_ICON);
    notify(webhook, message, &failure_emoji, true).await
}


/// Send notification to Slack with retry on failure
#[instrument]
pub async fn notify_success_with_retry(
    config: &Config,
    domain: &str,
    wildcard: bool,
) -> Result<(), SlackError> {
    // send success notification using a Slack webhook
    retry_async(
        Retry {
            max_tries: DEFAULT_MAX_NOTIFICATION_RETRIES,
            delay: Some(Delay::Static {
                delay: Duration::from_millis(DEFAULT_NOTIFICATION_RETRT_PAUSE_MS),
            }),
        },
        TokioSleep {},
        move || {
            async move {
                let slack_webhook = &config.slack_webhook().await;
                let message = if wildcard {
                    format!("Certificate renewal succeeded for the domain: *.{domain}.")
                } else {
                    format!("Certificate renewal succeeded for the domain: {domain}.")
                };
                notify_success(&slack_webhook.to_owned(), &message).await
            }
        },
    )
    .await
}


#[instrument]
pub async fn notify_telegram(
    chat_id: ChatId,
    token: &str,
    message: &str,
    icon: &str,
    fail: bool,
) -> Result<(), SlackError> {
    if token.is_empty() {
        warn!("Telegram Token undefined. Notifications will not be sent.");
        return Ok(());
    }
    let botapi = BotApi::new(String::from(token), None).await.unwrap();
    let send_message = SendMessage::new(chat_id, String::from(message));
    botapi.send_message(send_message).await.unwrap();
    Ok(())
}


/// Sends generic notification over Slack
#[instrument]
async fn notify(
    webhook: &str,
    message: &str,
    icon: &str,
    fail: bool,
) -> Result<(), SlackError> {
    if webhook.is_empty() {
        warn!("Webhook undefined. Notifications will not be sent.");
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

    slack.send(&payload).await
}
