#![allow(dead_code)]

use std::fmt;

use chrono::{DateTime, Utc};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    address::AddressError,
    message::{
        Attachment, Mailbox, Message, MultiPart, SinglePart,
        header::{ContentType, ContentTypeErr},
    },
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
        ends_at: DateTime<Utc>,
        timezone: &str,
        location_text: Option<&str>,
        location_url: Option<&str>,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::RegistrationConfirmation {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            event_title: event_title.to_owned(),
            starts_at,
            ends_at,
            timezone: timezone.to_owned(),
            location_text: location_text.map(ToOwned::to_owned),
            location_url: location_url.map(ToOwned::to_owned),
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

    pub async fn send_event_announcement_email(
        &self,
        to_email: &str,
        display_name: &str,
        event_title: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), EmailError> {
        self.enqueue(EmailTemplate::EventAnnouncement {
            to_email: to_email.to_owned(),
            display_name: display_name.to_owned(),
            event_title: event_title.to_owned(),
            subject: subject.to_owned(),
            body: body.to_owned(),
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
        ends_at: DateTime<Utc>,
        timezone: String,
        location_text: Option<String>,
        location_url: Option<String>,
    },
    EventReminder {
        to_email: String,
        display_name: String,
        event_title: String,
        starts_at: DateTime<Utc>,
        location: Option<String>,
    },
    EventAnnouncement {
        to_email: String,
        display_name: String,
        event_title: String,
        subject: String,
        body: String,
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

fn render_message(
    sender_mailbox: &Mailbox,
    template: EmailTemplate,
) -> Result<Message, EmailError> {
    let rendered = template.render();
    let to_mailbox = Mailbox::new(
        Some(rendered.recipient_name.clone()),
        rendered
            .to_email
            .parse()
            .map_err(EmailError::InvalidRecipientAddress)?,
    );

    let body = MultiPart::alternative()
        .singlepart(SinglePart::plain(rendered.text_body))
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(rendered.html_body),
        );
    let builder = Message::builder()
        .from(sender_mailbox.clone())
        .to(to_mailbox)
        .subject(rendered.subject);

    if rendered.attachments.is_empty() {
        return builder.multipart(body).map_err(EmailError::MessageBuild);
    }

    let mut mixed_body = MultiPart::mixed().multipart(body);
    for attachment in rendered.attachments {
        let content_type =
            ContentType::parse(&attachment.content_type).map_err(EmailError::InvalidContentType)?;
        mixed_body = mixed_body.singlepart(
            Attachment::new(attachment.filename).body(attachment.body, content_type),
        );
    }

    builder.multipart(mixed_body).map_err(EmailError::MessageBuild)
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
    attachments: Vec<RenderedAttachment>,
}

#[derive(Debug)]
struct RenderedAttachment {
    filename: String,
    content_type: String,
    body: String,
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
                    attachments: Vec::new(),
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
                    attachments: Vec::new(),
                }
            }
            Self::RegistrationConfirmation {
                to_email,
                display_name,
                event_title,
                starts_at,
                ends_at,
                timezone,
                location_text,
                location_url,
            } => {
                let subject = format!("Registration confirmed for {event_title}");
                let location =
                    location_description(location_text.as_deref(), location_url.as_deref());
                let location_line = location
                    .as_deref()
                    .map(|value| format!("Location: {value}\n"))
                    .unwrap_or_default();
                let location_html = location
                    .as_deref()
                    .map(|value| format!("<p>Location: {}</p>", escape_html(value)))
                    .unwrap_or_default();
                let text_body = format!(
                    concat!(
                        "Hi {},\n\n",
                        "Your registration for {} is confirmed.\n",
                        "Starts: {} UTC\n",
                        "Ends: {} UTC\n",
                        "Timezone: {}\n",
                        "{}",
                    ),
                    display_name,
                    event_title,
                    starts_at,
                    ends_at,
                    timezone,
                    location_line,
                );
                let html_body = format!(
                    concat!(
                        "<p>Hi {},</p>",
                        "<p>Your registration for <strong>{}</strong> is confirmed.</p>",
                        "<p>Starts: {} UTC<br>",
                        "Ends: {} UTC<br>",
                        "Timezone: {}</p>",
                        "{}",
                    ),
                    escape_html(&display_name),
                    escape_html(&event_title),
                    starts_at,
                    ends_at,
                    escape_html(&timezone),
                    location_html,
                );
                let calendar_body = build_registration_calendar(
                    &event_title,
                    starts_at,
                    ends_at,
                    location.as_deref(),
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                    attachments: vec![RenderedAttachment {
                        filename: calendar_filename(&event_title),
                        content_type: "text/calendar; charset=utf-8; method=REQUEST".to_owned(),
                        body: calendar_body,
                    }],
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
                    attachments: Vec::new(),
                }
            }
            Self::EventAnnouncement {
                to_email,
                display_name,
                event_title,
                subject,
                body,
            } => {
                let subject = format!("{event_title}: {subject}");
                let text_body = format!("Hi {display_name},\n\n{body}\n");
                let html_body = format!(
                    "<p>Hi {},</p><p>{}</p>",
                    escape_html(&display_name),
                    escape_html(&body).replace('\n', "<br>")
                );

                RenderedEmail {
                    to_email,
                    recipient_name: display_name,
                    subject,
                    text_body,
                    html_body,
                    attachments: Vec::new(),
                }
            }
        }
    }
}

fn location_description(location_text: Option<&str>, location_url: Option<&str>) -> Option<String> {
    match (location_text, location_url) {
        (Some(text), Some(url)) if !text.trim().is_empty() && !url.trim().is_empty() => {
            Some(format!("{} ({})", text.trim(), url.trim()))
        }
        (Some(text), _) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        (_, Some(url)) if !url.trim().is_empty() => Some(url.trim().to_owned()),
        _ => None,
    }
}

fn calendar_filename(event_title: &str) -> String {
    let slug = event_title
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character.is_whitespace() || matches!(character, '-' | '_') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "event.ics".to_owned()
    } else {
        format!("{slug}.ics")
    }
}

fn build_registration_calendar(
    event_title: &str,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    location: Option<&str>,
) -> String {
    let location_line = location
        .map(|value| format!("LOCATION:{}\r\n", escape_ics_text(value)))
        .unwrap_or_default();

    format!(
        concat!(
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "PRODID:-//Mike T NMPF//Event Organization//EN\r\n",
            "CALSCALE:GREGORIAN\r\n",
            "METHOD:REQUEST\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:{}-{}@mike-t-nmpf-event-organization\r\n",
            "DTSTAMP:{}\r\n",
            "DTSTART:{}\r\n",
            "DTEND:{}\r\n",
            "SUMMARY:{}\r\n",
            "{}",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n"
        ),
        starts_at.timestamp(),
        escape_ics_uid(event_title),
        format_ics_datetime(Utc::now()),
        format_ics_datetime(starts_at),
        format_ics_datetime(ends_at),
        escape_ics_text(event_title),
        location_line,
    )
}

fn format_ics_datetime(value: DateTime<Utc>) -> String {
    value.format("%Y%m%dT%H%M%SZ").to_string()
}

fn escape_ics_uid(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
}

fn escape_ics_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[derive(Debug)]
pub enum EmailError {
    InvalidFromAddress(AddressError),
    InvalidRecipientAddress(AddressError),
    InvalidContentType(ContentTypeErr),
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
            Self::InvalidContentType(error) => {
                write!(formatter, "invalid attachment content type: {error}")
            }
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
    use super::{EmailTemplate, RenderedEmail, render_message};
    use chrono::{TimeZone, Utc};
    use lettre::message::Mailbox;

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

    #[test]
    fn registration_confirmation_template_includes_calendar_attachment() {
        let rendered = render(EmailTemplate::RegistrationConfirmation {
            to_email: "user@example.com".to_owned(),
            display_name: "User".to_owned(),
            event_title: "Annual Gala".to_owned(),
            starts_at: Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap(),
            ends_at: Utc.with_ymd_and_hms(2026, 5, 18, 14, 0, 0).unwrap(),
            timezone: "America/New_York".to_owned(),
            location_text: Some("Main Hall".to_owned()),
            location_url: Some("https://example.com/live".to_owned()),
        });

        assert!(rendered.subject.contains("Annual Gala"));
        assert!(rendered.text_body.contains("America/New_York"));
        assert_eq!(rendered.attachments.len(), 1);
        assert_eq!(rendered.attachments[0].filename, "annual-gala.ics");
        assert!(rendered.attachments[0].body.contains("BEGIN:VCALENDAR"));
        assert!(rendered.attachments[0].body.contains("SUMMARY:Annual Gala"));
        assert!(
            rendered.attachments[0]
                .body
                .contains("LOCATION:Main Hall (https://example.com/live)")
        );
    }

    #[test]
    fn registration_confirmation_message_renders_ics_mime_part() {
        let sender = Mailbox::new(
            Some("Events".to_owned()),
            "events@example.com".parse().expect("sender should parse"),
        );
        let message = render_message(
            &sender,
            EmailTemplate::RegistrationConfirmation {
                to_email: "user@example.com".to_owned(),
                display_name: "User".to_owned(),
                event_title: "Annual Gala".to_owned(),
                starts_at: Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap(),
                ends_at: Utc.with_ymd_and_hms(2026, 5, 18, 14, 0, 0).unwrap(),
                timezone: "America/New_York".to_owned(),
                location_text: Some("Main Hall".to_owned()),
                location_url: None,
            },
        )
        .expect("registration confirmation should render");
        let formatted =
            String::from_utf8(message.formatted()).expect("message should be utf-8");

        assert!(formatted.contains("Content-Type: text/calendar"));
        assert!(formatted.contains("filename=\"annual-gala.ics\""));
        assert!(formatted.contains("BEGIN:VCALENDAR"));
    }

    #[test]
    fn announcement_template_escapes_html_body() {
        let rendered = render(EmailTemplate::EventAnnouncement {
            to_email: "user@example.com".to_owned(),
            display_name: "User".to_owned(),
            event_title: "Gala".to_owned(),
            subject: "Update".to_owned(),
            body: "<script>alert(1)</script>".to_owned(),
        });

        assert!(rendered.subject.contains("Gala"));
        assert!(rendered.text_body.contains("<script>alert(1)</script>"));
        assert!(rendered.html_body.contains("&lt;script&gt;"));
    }
}
