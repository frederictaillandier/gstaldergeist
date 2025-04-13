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

pub fn request_new_bags() {
    let email_config = EmailConfig::from_env().unwrap();

    let res = send_email(
        &email_config,
        email_config.to_email.as_str(),
        "We Recycle",
        "Request for new bags",
        format!(
            "Hi!\nMy name is {} and my address is {}\nUnfortunately It looks like we do not have any bags anymore ?\nCould you please send us some new ones ?\nThank you very much !\n{}",
            email_config.from_name, email_config.address, email_config.from_name
        ).as_str(),
        Some(
            format!(
                "<p>Hi,</p>
                <p>My name is {} and my address is {}</p>
        <p>Unfortunately It looks like we do not have any bags anymore ?</p>
        <p>Could you please send us some new ones ?</p>
        <p>Thank you very much !</p>
        <p>{}</p>
        ",
                email_config.from_name, email_config.address, email_config.from_name
            )
            .as_str(),
        ),
    );
    match res {
        Ok(_) => println!("Email sent successfully"),
        Err(e) => eprintln!("Error sending email: {}", e),
    }
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
