#![allow(dead_code)]

use std::fmt;

use chrono::{DateTime, Utc};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    address::AddressError,
    message::{Mailbox, Message, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        Error as SmtpTransportError,
        authentication::Credentials,
        response::Response,
    },
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::SmtpConfig;

const EMAIL_QUEUE_CAPACITY: usize = 128;

#[derive(Clone)]
pub struct EmailService {
    sender: mpsc::Sender<EmailJob>,
}

impl EmailService {
    pub fn spawn(config: &SmtpConfig) -> Result<Self, EmailError> {
        let transport = build_transport(config)?;
        let sender_mailbox = Mailbox::new(
            Some(config.from_name.clone()),
            config
                .from_email
                .parse()
                .map_err(EmailError::InvalidFromAddress)?,
        );
        let (sender, receiver) = mpsc::channel(EMAIL_QUEUE_CAPACITY);

        tokio::spawn(email_worker(receiver, transport, sender_mailbox));

        Ok(Self { sender })
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        display_name: &str,
        verification_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::Verification {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            verification_token: verification_token.to_owned(),
            expires_at,
        })
        .await
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        display_name: &str,
        reset_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::PasswordReset {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            reset_token: reset_token.to_owned(),
            expires_at,
        })
        .await
    }

    pub async fn send_registration_confirmation_email(
        &self,
        to_email: &str,
        display_name: &str,
        event_title: &str,
        starts_at: DateTime<Utc>,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::RegistrationConfirmation {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            event_title: event_title.to_owned(),
            starts_at,
        })
        .await
    }

    pub async fn send_event_reminder_email(
        &self,
        to_email: &str,
        display_name: &str,
        event_title: &str,
        starts_at: DateTime<Utc>,
        location: Option<&str>,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::EventReminder {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            event_title: event_title.to_owned(),
            starts_at,
            location: location.map(ToOwned::to_owned),
        })
        .await
    }

    async fn enqueue(&self, template: EmailTemplate) -> Result<(), EmailError> {
        self.sender
            .try_send(EmailJob { template })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => EmailError::QueueFull,
                mpsc::error::TrySendError::Closed(_) => EmailError::QueueClosed,
            })
    }
}

#[derive(Debug)]
struct EmailJob {
    template: EmailTemplate,
}

#[derive(Debug)]
enum EmailTemplate {
    Verification {
        to_email: String,
        display_name: String,
        verification_token: String,
        expires_at: DateTime<Utc>,
    },
    PasswordReset {
        to_email: String,
        display_name: String,
        reset_token: String,
        expires_at: DateTime<Utc>,
    },
    RegistrationConfirmation {
        to_email: String,
        display_name: String,
        event_title: String,
        starts_at: DateTime<Utc>,
    },
    EventReminder {
        to_email: String,
        display_name: String,
        event_title: String,
        starts_at: DateTime<Utc>,
        location: Option<String>,
    },
}

async fn email_worker(
    mut receiver: mpsc::Receiver<EmailJob>,
    transport: AsyncSmtpTransport<Tokio1Executor>,
    sender_mailbox: Mailbox,
) {
    while let Some(job) = receiver.recv().await {
        let message = match render_message(&sender_mailbox, job.template) {
            Ok(message) => message,
            Err(error) => {
                error!(error = %error, "failed to render transactional email");
                continue;
            }
        };

        match transport.send(message).await {
            Ok(response) => log_delivery(response),
            Err(error) => error!(error = %error, "failed to send transactional email"),
        }
    }

    warn!("transactional email worker exited because the queue was closed");
}

fn build_transport(config: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, EmailError> {
    let credentials = Credentials::new(config.username.clone(), config.password.clone());
    let builder = if config.use_starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(EmailError::TransportConfig)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
    };

    Ok(builder
        .port(config.port)
        .credentials(credentials)
        .build())
}

fn render_message(sender_mailbox: &Mailbox, template: EmailTemplate) -> Result<Message, EmailError> {
    let rendered = template.render();
    let to_mailbox = Mailbox::new(
        Some(rendered.recipient_name.clone()),
        rendered
            .to_email
            .parse()
            .map_err(EmailError::InvalidRecipientAddress)?,
    );

    Message::builder()
        .from(sender_mailbox.clone())
        .to(to_mailbox)
        .subject(rendered.subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(rendered.text_body))
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(rendered.html_body),
                ),
        )
        .map_err(EmailError::MessageBuild)
}

fn log_delivery(response: Response) {
    info!(smtp_response = ?response, "transactional email sent");
}

#[derive(Debug)]
struct RenderedEmail {
    to_email: String,
    recipient_name: String,
    subject: String,
    text_body: String,
    html_body: String,
}

impl EmailTemplate {
    fn render(self) -> RenderedEmail {
        match self {
            Self::Verification {
                to_email,
                display_name,
                verification_token,
                expires_at,
            } => {
                let subject = "Verify your email address".to_owned();
                let text_body = format!(
                    "Hi {display_name},\n\nUse this verification token to confirm your account:\n\n{verification_token}\n\nThis token expires at {expires_at} UTC.\n"
                );
                let html_body = format!(
                    "<p>Hi {display_name},</p><p>Use this verification token to confirm your account:</p><p><strong>{verification_token}</strong></p><p>This token expires at {expires_at} UTC.</p>"
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                }
            }
            Self::PasswordReset {
                to_email,
                display_name,
                reset_token,
                expires_at,
            } => {
                let subject = "Reset your password".to_owned();
                let text_body = format!(
                    "Hi {display_name},\n\nUse this password reset token to set a new password:\n\n{reset_token}\n\nThis token expires at {expires_at} UTC.\n"
                );
                let html_body = format!(
                    "<p>Hi {display_name},</p><p>Use this password reset token to set a new password:</p><p><strong>{reset_token}</strong></p><p>This token expires at {expires_at} UTC.</p>"
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                }
            }
            Self::RegistrationConfirmation {
                to_email,
                display_name,
                event_title,
                starts_at,
            } => {
                let subject = format!("Registration confirmed for {event_title}");
                let text_body = format!(
                    "Hi {display_name},\n\nYour registration for {event_title} is confirmed.\nThe event starts at {starts_at} UTC.\n"
                );
                let html_body = format!(
                    "<p>Hi {display_name},</p><p>Your registration for <strong>{event_title}</strong> is confirmed.</p><p>The event starts at {starts_at} UTC.</p>"
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                }
            }
            Self::EventReminder {
                to_email,
                display_name,
                event_title,
                starts_at,
                location,
            } => {
                let subject = format!("Reminder: {event_title} is coming up");
                let location_line = location
                    .as_deref()
                    .map(|value| format!("Location: {value}\n"))
                    .unwrap_or_default();
                let location_html = location
                    .as_deref()
                    .map(|value| format!("<p>Location: {value}</p>"))
                    .unwrap_or_default();
                let text_body = format!(
                    "Hi {display_name},\n\nThis is a reminder that {event_title} starts at {starts_at} UTC.\n{location_line}"
                );
                let html_body = format!(
                    "<p>Hi {display_name},</p><p>This is a reminder that <strong>{event_title}</strong> starts at {starts_at} UTC.</p>{location_html}"
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum EmailError {
    InvalidFromAddress(AddressError),
    InvalidRecipientAddress(AddressError),
    MessageBuild(lettre::error::Error),
    QueueFull,
    QueueClosed,
    TransportConfig(SmtpTransportError),
}

impl fmt::Display for EmailError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFromAddress(error) => write!(formatter, "invalid SMTP from address: {error}"),
            Self::InvalidRecipientAddress(error) => write!(formatter, "invalid recipient address: {error}"),
            Self::MessageBuild(error) => write!(formatter, "failed to build transactional email: {error}"),
            Self::QueueFull => formatter.write_str("transactional email queue is full"),
            Self::QueueClosed => formatter.write_str("transactional email queue is closed"),
            Self::TransportConfig(error) => write!(formatter, "failed to configure SMTP transport: {error}"),
        }
    }
}

impl std::error::Error for EmailError {}

#[cfg(test)]
mod tests {
    use super::{EmailTemplate, RenderedEmail};
    use chrono::{TimeZone, Utc};

    fn render(template: EmailTemplate) -> RenderedEmail {
        template.render()
    }

    #[test]
    fn verification_template_contains_token() {
        let rendered = render(EmailTemplate::Verification {
            to_email: "user@example.com".to_owned(),
            display_name: "User".to_owned(),
            verification_token: "verification-token".to_owned(),
            expires_at: Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap(),
        });

        assert!(rendered.subject.contains("Verify"));
        assert!(rendered.text_body.contains("verification-token"));
        assert!(rendered.html_body.contains("verification-token"));
    }

    #[test]
    fn password_reset_template_contains_token() {
        let rendered = render(EmailTemplate::PasswordReset {
            to_email: "user@example.com".to_owned(),
            display_name: "User".to_owned(),
            reset_token: "reset-token".to_owned(),
            expires_at: Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap(),
        });

        assert!(rendered.subject.contains("Reset"));
        assert!(rendered.text_body.contains("reset-token"));
        assert!(rendered.html_body.contains("reset-token"));
    }
}
