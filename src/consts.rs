/// Max retries for ACME query
pub const DEFAULT_MAX_ATTEMPTS: usize = 5;

/// Max retries for notifications
pub const DEFAULT_MAX_NOTIFICATION_RETRIES: usize = 5;

/// When notification fails, pause this amount of time before trying again
pub const DEFAULT_NOTIFICATION_RETRT_PAUSE_MS: u64 = 5000;

/// ACME poll time when awaiting for the certificate
pub const DEFAULT_ACME_POLL_PAUSE_MS: u64 = 5000;

/// Pause to await when ACME API responds with an "Invalid" state
pub const DEFAULT_ACME_INVALID_STATUS_PAUSE_MS: u64 = 30000;

/// Default Notification name:
pub const DEFAULT_SLACK_NAME: &str = "CertsD";

/// Default Notification failure icon:
pub const DEFAULT_SLACK_FAILURE_ICON: &str = ":error:";

/// Default Notification success icon:
pub const DEFAULT_SLACK_SUCCESS_ICON: &str = ":white_check_mark:";

/// Default failure notification color:
pub const DEFAULT_SLACK_FAILURE_COLOR: &str = "#ff1111";

/// Default success notification color:
pub const DEFAULT_SLACK_SUCCESS_COLOR: &str = "#00ff00";
