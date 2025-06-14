<div align="center">
  <img src="https://github.com/user-attachments/assets/bcc3cd27-5008-4491-8c53-0b12a35f00c5" alt="Alt text" width="300" height="300">
</div>


# Gstaldergeist

A rust telegram chatbot to manage a shared house's chores.
It assigns to a flatmate the role of trash dispenser for a week by senting reminder everyday.
If collects data from the city of Adliswil and We-Recycle.


## Install and Use

With podman
```
podman pull ghcr.io/frederictaillandier/gstaldergeist
podman run -d --env-file .env gstaldergeist
```
or
```
cargo run --release
```

For custome configuration these env variables are expected
```
export TELEGRAM_BOT_TOKEN="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
export TELEGRAM_CHANNEL_ID="-654654654654"
export TELEGRAM_FLATMATES="-654654654654, -654654654654, -654654654654"
export EMAIL_SMTP_SERVER="smtp.gmail.com"
export EMAIL_ADDRESS="gstaldergeist@gmail.com"
export EMAIL_PASSWORD="xxxx xxxx xxxx xxxx"
export EMAIL_NAME="Gstalder Geist"
export ADDRESS="Gstalderstreet 1 9999 Gstaldercity"
export TO_EMAIL="we-recycle@gmail.com"
```

using these parameters:
**TELEGRAM_BOT_TOKEN** is your [telegram](https://core.telegram.org/bots/api) bot token</br>
**TELEGRAM_CHANNEL_ID** is the [telegram](https://core.telegram.org/bots/api) group chat where the bot writes for all flatmates</br>
**TELEGRAM_FLATMATES** is the list of your flatmates and **chat_it** fits a chat only targeted for a specific flatmate</br>

run then with
```cargo run --release```

## Production environment
The main version of the bot is running on a european cloud provider [KIMSUFI](https://www.kimsufi.com/en/)

![image](https://github.com/frederictaillandier/GstalderBot/assets/5926779/96835696-8428-4a25-8309-3a1ea17c90b8)
![image](https://github.com/frederictaillandier/GstalderBot/assets/5926779/733c27bb-086e-4016-ab94-35e8820a77bc)
