<div align="center">
  <img src="https://github.com/user-attachments/assets/d5561eec-ba3c-4c98-af29-0d8744e6d53d" alt="Alt text" width="300" height="300">
</div>
# Gstaldergeist

A rust telegram chatbot to manage a shared house's chores.
It assigns to a flatmate the role of trash dispenser for a week by senting reminder everyday.
If collects data from the city of Adliswil and We-Recycle
<img src="https://github.com/user-attachments/assets/b10e2eaa-3490-41bf-bd25-145dda28d3cf" alt="Alt text" width="256" height="90">
<img src="https://github.com/user-attachments/assets/02357792-9efe-4263-b59b-6fbe9d0df23d" alt="Alt text" width="256" height="90">

## Install and Use

For custome configuration these env variables are expected

`GSTALDERCONFIG_PROD`
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
export NB_REMINDERS="3"
export DELTA_REMINDER_SEC="10"
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


