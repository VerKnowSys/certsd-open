use crate::*;

use retry::{delay::Fixed, retry_with_index, OperationResult};
use slack_hooked::{AttachmentBuilder, PayloadBuilder, Slack};

/// Default Notification name:
const DEFAULT_SLACK_NAME: &str = "CertsD";

/// Default Notification failure icon:
const DEFAULT_SLACK_FAILURE_ICON: &str = ":error:";

/// Default Notification success icon:
const DEFAULT_SLACK_SUCCESS_ICON: &str = ":white_check_mark:";

/// Default failure notification color:
const DEFAULT_SLACK_FAILURE_COLOR: &str = "#ff1111";

/// Default success notification color:
const DEFAULT_SLACK_SUCCESS_COLOR: &str = "#00ff00";


/// Sends success notification to Slack
#[instrument]
pub fn notify_success(webhook: &str, message: &str) {
    let success_emoji = String::from(DEFAULT_SLACK_SUCCESS_ICON);
    notify(webhook, message, &success_emoji, false)
}


/// Sends failure notification to Slack
#[instrument]
pub fn notify_failure(webhook: &str, message: &str) {
    let failure_emoji = String::from(DEFAULT_SLACK_FAILURE_ICON);
    notify(webhook, message, &failure_emoji, true)
}


/// Sends generic notification over Slack
#[instrument]
fn notify(webhook: &str, message: &str, icon: &str, fail: bool) {
    if webhook.is_empty() {
        warn!("Webhook undefined. Notifications will not be sent.");
        return;
    }
    retry_with_index(Fixed::from_millis(1000), |current_try| {
        if current_try > 3 {
            return OperationResult::Err("Did not succeed within 3 tries");
        }

        let notification = Slack::new(webhook).and_then(|slack| {
            PayloadBuilder::new()
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
                .build()
                .and_then(|payload| {
                    debug!("Sending notification with payload: {payload:?}");
                    slack.send(&payload)
                })
        });

        match notification {
            Ok(_) => OperationResult::Ok("Sent!"),
            Err(_) => OperationResult::Retry("Failed to send notification!"),
        }
    })
    .map_err(|err| {
        error!("Error sending notification: {err}");
        err
    })
    .unwrap_or_default();
}
