use lettre::message::{MultiPart, SinglePart, header};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::error::Error;

pub struct EmailConfig {
    smtp_server: String,
    smtp_port: u16,
    username: String,
    password: String,
    from_email: String,
    from_name: String,
    address: String,
    to_email: String,
}

impl EmailConfig {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            smtp_server: std::env::var("EMAIL_SMTP_SERVER")?,
            smtp_port: 465,
            username: std::env::var("EMAIL_ADDRESS")?,
            password: std::env::var("EMAIL_PASSWORD")?,
            from_email: std::env::var("EMAIL_ADDRESS")?,
            from_name: std::env::var("EMAIL_NAME")?,
            address: std::env::var("ADDRESS")?,
            to_email: std::env::var("TO_EMAIL")?,
        })
    }
}

fn bag_request_text_body(name: &str, address: &str) -> String {
    format!(
        "Hi!\n\
         My name is {name} and my address is {address}\n\
         Unfortunately It looks like we do not have any bags anymore ?\n\
         Could you please send us some new ones ?\n\
         Thank you very much !\n\
         {name}"
    )
}

fn bag_request_html_body(name: &str, address: &str) -> String {
    format!(
        "<p>Hi,</p>\n\
         <p>My name is {name} and my address is {address}</p>\n\
         <p>Unfortunately It looks like we do not have any bags anymore ?</p>\n\
         <p>Could you please send us some new ones ?</p>\n\
         <p>Thank you very much !</p>\n\
         <p>{name}</p>\n"
    )
}

pub fn request_new_bags() -> Result<(), Box<dyn Error>> {
    let email_config = EmailConfig::from_env()?;

    send_email(
        &email_config,
        email_config.to_email.as_str(),
        "We Recycle",
        "Request for new bags",
        bag_request_text_body(&email_config.from_name, &email_config.address).as_str(),
        Some(bag_request_html_body(&email_config.from_name, &email_config.address).as_str()),
    )?;
    tracing::info!("Email sent successfully");
    Ok(())
}

fn send_email(
    config: &EmailConfig,
    to_email: &str,
    to_name: &str,
    subject: &str,
    text_body: &str,
    html_body: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    // Create email
    let email_builder = Message::builder()
        .from(format!("{} <{}>", config.from_name, config.from_email).parse()?)
        .to(format!("{} <{}>", to_name, to_email).parse()?)
        .subject(subject);

    // Add message body (with HTML alternative if provided)
    let email = if let Some(html) = html_body {
        email_builder.multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(text_body.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(html.to_string()),
                ),
        )?
    } else {
        email_builder.body(text_body.to_string())?
    };

    // Set up credentials
    let creds = Credentials::new(config.username.clone(), config.password.clone());

    // Set up and use the SMTP transport
    let mailer = SmtpTransport::relay(&config.smtp_server)?
        .port(config.smtp_port)
        .credentials(creds)
        .build();

    // Send the email
    mailer.send(&email)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bag_request_html_body, bag_request_text_body};

    #[test]
    fn text_body_interpolates_name_and_address() {
        let body = bag_request_text_body("Alice", "1 Main St");
        assert!(body.contains("My name is Alice and my address is 1 Main St"));
    }

    #[test]
    fn text_body_signs_off_with_the_name() {
        let body = bag_request_text_body("Alice", "1 Main St");
        assert!(body.ends_with("Alice"));
    }

    #[test]
    fn html_body_wraps_each_line_in_a_paragraph() {
        let body = bag_request_html_body("Alice", "1 Main St");
        assert!(body.contains("<p>My name is Alice and my address is 1 Main St</p>"));
        assert!(body.starts_with("<p>Hi,</p>"));
        assert!(body.trim_end().ends_with("<p>Alice</p>"));
    }

    #[test]
    fn bodies_carry_the_same_request() {
        // Both alternatives must say the same thing so a client showing either
        // one sees a coherent request.
        for body in [
            bag_request_text_body("Bob", "2 Side Rd"),
            bag_request_html_body("Bob", "2 Side Rd"),
        ] {
            assert!(body.contains("Bob"));
            assert!(body.contains("2 Side Rd"));
            assert!(body.contains("send us some new ones"));
        }
    }
}
