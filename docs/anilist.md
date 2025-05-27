# Anilist integration (as of v0.5.0)

## Steps to set it up (Method 1)

1. Login to your anilist account and go to Settings / Developer / Create new client
Name it whatever you like and put in `Redirect URL`the following : `https://anilist.co/api/v2/oauth/pin`
![image](https://github.com/user-attachments/assets/e0b1ece6-bbee-441e-9d09-0042f6a85ea8)

2. Run this command to provide your access token, follow the instructions and make sure you are logged in to your anilist account

```shell
./manga-tui  anilist init
```

3. Run this command to check if everything is setup correctly

```shell
./manga-tui  anilist check
```

4. Now just run `./manga-tui` and read manga as always, you should see your reading history being updated in your anilist account


## If you can't provide your anilist credentials from the terminal following Method 1

Provide both `client_id` and `access_token` in the `config.toml`

The config file is located at `XDG_CONFIG_HOME/manga-tui/config.toml`, to know where it is you can run:

```shell
manga-tui --config-dir 

# or

manga-tui -c
```

```toml
# Enable / disable tracking reading history with services like `anilist`
# values: true, false
# default: true
track_reading_history = true

# ...
# Anilist-related config, if you want `manga-tui` to read your anilist credentials from this file then place them here
[anilist]
# Your client id from your anilist account
# leave it as an empty string "" if you don't want to use the config file to read your anilist credentials
# values: string
# default: ""
client_id = ""

# Your acces token from your anilist account
# leave it as an empty string "" if you don't want to use the config file to read your anilist credentials
# values: string
# default: ""
access_token = ""
```

if you wish to stop using your credentials from config file then leave either `client_id` or `access_token` empty
or set `track_reading_history` to `false` 
