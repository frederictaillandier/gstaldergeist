<div align="center">
  <img src="https://github.com/user-attachments/assets/bcc3cd27-5008-4491-8c53-0b12a35f00c5" alt="Alt text" width="300" height="300">
</div>

[![CI/CD](https://github.com/frederictaillandier/gstaldergeist/actions/workflows/deploy.yml/badge.svg)](https://github.com/frederictaillandier/gstaldergeist/actions/workflows/deploy.yml)
[![CI/CD](https://github.com/frederictaillandier/gstaldergeist/actions/workflows/deploy-prod.yml/badge.svg)](https://github.com/frederictaillandier/gstaldergeist/actions/workflows/deploy-prod.yml)

# Gstaldergeist

A Rust-based Telegram chatbot designed to manage household chores in a shared living environment. It automates the assignment of trash disposal duties among flatmates and provides daily reminders. The bot also integrates with local waste management services in Adliswil and We-Recycle.

## Features

- Automated chore assignment for trash disposal
- Daily reminders for assigned chores
- Integration with local waste management services
- Telegram-based communication system
- Email notifications for important updates
- Customizable configuration via environment variables

## Prerequisites

- Rust (if building from source)
- Podman/Docker (for containerized deployment)
- Telegram account and bot token
- Email service with SMTP support

## Installation

### Using Podman
```bash
podman pull ghcr.io/frederictaillandier/gstaldergeist
podman run -d --env-file .env gstaldergeist
```

### Building from Source
```bash
cargo run --release
```

## Configuration

Create a `.env` file with the following variables:

```bash
TELEGRAM_BOT_TOKEN="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
TELEGRAM_CHANNEL_ID="-654654654654"
TELEGRAM_FLATMATES="-654654654654, -654654654654, -654654654654"
EMAIL_SMTP_SERVER="smtp.gmail.com"
EMAIL_ADDRESS="gstaldergeist@gmail.com"
EMAIL_PASSWORD="xxxx xxxx xxxx xxxx"
EMAIL_NAME="Gstalder Geist"
ADDRESS="Gstalderstreet 1 9999 Gstaldercity"
TO_EMAIL="we-recycle@gmail.com"
```

### Configuration Parameters

- `TELEGRAM_BOT_TOKEN`: Your Telegram bot token (obtain from @BotFather)
- `TELEGRAM_CHANNEL_ID`: Group chat ID where the bot will post announcements
- `TELEGRAM_FLATMATES`: Comma-separated list of individual chat IDs for notifications
- `EMAIL_SMTP_SERVER`: SMTP server for email notifications
- `EMAIL_ADDRESS`: Email address used by the bot
- `EMAIL_PASSWORD`: Password for the email account
- `EMAIL_NAME`: Display name for email notifications
- `ADDRESS`: Your household address
- `TO_EMAIL`: Recipient email for We-Recycle notifications

## Usage

1. Create a `.env` file with your configuration
2. Run the bot using either Podman or build from source
3. The bot will automatically:
   - Assign chores weekly
   - Send daily reminders
   - Collect and process data from local waste management services

## Project Structure

```
src/
├── main.rs          # Main bot implementation
├── email.rs         # Email handling functionality
└── data_grabber/    # Data collection modules
    ├── we_recycle.rs
    └── adliswil.rs
```

## Production Environment

The bot is deployed on a European cloud server using KIMSUFI, ensuring reliable operation and minimal latency for European users.

![image](https://github.com/frederictaillandier/GstalderBot/assets/5926779/96835696-8428-4a25-8309-3a1ea17c90b8)
![image](https://github.com/frederictaillandier/GstalderBot/assets/5926779/733c27bb-086e-4016-ab94-35e8820a77bc)
