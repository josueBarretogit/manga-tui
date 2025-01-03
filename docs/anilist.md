# Anilist integration (as of v0.5.0)

## Steps to set it up 

1.  Login to your anilist account and go to Settings / Developer / Create new client
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
