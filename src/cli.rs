use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::io::BufRead;
use std::process::exit;

use clap::{crate_version, Parser, Subcommand};
use strum::IntoEnumIterator;

use crate::backend::error_log::write_to_error_log;
use crate::backend::filter::Languages;
use crate::backend::secrets::anilist::{AnilistCredentials, AnilistStorage};
use crate::backend::secrets::SecretStorage;
use crate::backend::tracker::anilist::{self, BASE_ANILIST_API_URL};
use crate::backend::APP_DATA_DIR;
use crate::global::PREFERRED_LANGUAGE;
use crate::logger::{ILogger, Logger};

fn read_input(mut input_reader: impl BufRead, logger: &impl ILogger, message: &str) -> Result<String, Box<dyn Error>> {
    logger.inform(message);
    let mut input_provided = String::new();
    input_reader.read_line(&mut input_provided)?;
    Ok(input_provided)
}

#[derive(Subcommand, Clone, Copy)]
pub enum AnilistCommand {
    /// setup anilist client to be able to sync reading progress
    Init,
    /// check wheter or not anilist is setup correctly
    Check,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    Lang {
        #[arg(short, long)]
        print: bool,
        #[arg(short, long)]
        set: Option<String>,
    },

    Anilist {
        #[command(subcommand)]
        command: AnilistCommand,
    },
}

#[derive(Parser, Clone)]
#[command(version = crate_version!())]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long)]
    pub data_dir: bool,
}

pub struct AnilistCredentialsProvided<'a> {
    pub access_token: &'a str,
    pub client_id: &'a str,
}

impl CliArgs {
    pub fn new() -> Self {
        Self {
            command: None,
            data_dir: false,
        }
    }

    pub fn with_command(mut self, command: Commands) -> Self {
        self.command = Some(command);
        self
    }

    pub fn print_available_languages() {
        println!("The available languages are:");
        Languages::iter().filter(|lang| *lang != Languages::Unkown).for_each(|lang| {
            println!("{} {} | iso code : {}", lang.as_emoji(), lang.as_human_readable().to_lowercase(), lang.as_iso_code())
        });
    }

    pub fn init_anilist(
        &self,
        mut input_reader: impl BufRead,
        storage: &mut impl SecretStorage,
        logger: impl ILogger,
    ) -> Result<(), Box<dyn Error>> {
        let client_id = read_input(&mut input_reader, &logger, "Provide your client id")?;
        let client_id = client_id.trim();

        let anilist_retrieve_access_token_url =
            format!("https://anilist.co/api/v2/oauth/authorize?client_id={client_id}&response_type=token");

        let open_in_browser_message = format!("Opening {anilist_retrieve_access_token_url}  to get the access token ");

        logger.inform(open_in_browser_message);

        open::that(anilist_retrieve_access_token_url)?;

        let access_token = read_input(&mut input_reader, &logger, "Enter the access token")?;
        let access_token = access_token.trim();

        self.save_anilist_credentials(
            AnilistCredentialsProvided {
                access_token,
                client_id,
            },
            storage,
        )?;

        logger.inform("Anilist was correctly setup :D");

        Ok(())
    }

    fn save_anilist_credentials(
        &self,
        credentials: AnilistCredentialsProvided<'_>,
        storage: &mut impl SecretStorage,
    ) -> Result<(), Box<dyn Error>> {
        storage.save_multiple_secrets(HashMap::from([
            (AnilistCredentials::AccessToken.to_string(), credentials.access_token.to_string()),
            (AnilistCredentials::ClientId.to_string(), credentials.client_id.to_string()),
        ]))?;
        Ok(())
    }

    async fn check_anilist_token(&self, token_checker: &impl AnilistTokenChecker, token: String) -> Result<bool, Box<dyn Error>> {
        token_checker.verify_token(token).await
    }

    async fn check_anilist_status(&self, logger: &impl ILogger) -> Result<(), Box<dyn Error>> {
        let storage = AnilistStorage::new();
        logger.inform("Checking client id and access token are stored");

        let credentials_are_stored = storage.check_credentials_stored()?;
        if credentials_are_stored.is_none() {
            logger.warn("The client id or the access token are empty, run `manga-tui anilist init`");
            exit(0)
        }

        let credentials = credentials_are_stored.unwrap();
        logger.inform("Checking your access token is valid, this may take a while");

        let anilist = anilist::Anilist::new(BASE_ANILIST_API_URL.parse().unwrap())
            .with_token(credentials.access_token.clone())
            .with_client_id(credentials.client_id);

        let access_token_is_valid = self.check_anilist_token(&anilist, credentials.access_token).await?;

        if access_token_is_valid {
            logger.inform("Everything is setup correctly :D");
        } else {
            logger.error("The anilist access token is not valid, please run `manga-tui anilist init`".into());
            exit(0)
        }

        Ok(())
    }

    /// This method should only return `Ok(())` it the app should keep running, otherwise `exit`
    pub async fn proccess_args(self) -> Result<(), Box<dyn Error>> {
        if self.data_dir {
            let app_dir = APP_DATA_DIR.as_ref().unwrap();
            println!("{}", app_dir.to_str().unwrap());
            exit(0)
        }

        match &self.command {
            Some(command) => match command {
                Commands::Lang { print, set } => {
                    if *print {
                        Self::print_available_languages();
                        exit(0)
                    }

                    match set {
                        Some(lang) => {
                            let try_lang = Languages::try_from_iso_code(lang.as_str());

                            if try_lang.is_none() {
                                println!(
                                    "`{}` is not a valid ISO language code, run `{} lang --print` to list available languages and their ISO codes",
                                    lang,
                                    env!("CARGO_BIN_NAME")
                                );

                                exit(0)
                            }

                            PREFERRED_LANGUAGE.set(try_lang.unwrap()).unwrap();
                        },
                        None => {
                            PREFERRED_LANGUAGE.set(Languages::default()).unwrap();
                        },
                    }
                    Ok(())
                },

                Commands::Anilist { command } => match command {
                    AnilistCommand::Init => {
                        let mut storage = AnilistStorage::new();
                        self.init_anilist(std::io::stdin().lock(), &mut storage, Logger)?;
                        exit(0)
                    },
                    AnilistCommand::Check => {
                        let logger = Logger;
                        if let Err(e) = self.check_anilist_status(&logger).await {
                            logger.error(format!("Some error ocurred, more details \n {}", e).into());
                            write_to_error_log(e.into());
                            exit(1);
                        } else {
                            exit(0)
                        }
                    },
                },
            },
            None => {
                PREFERRED_LANGUAGE.set(Languages::default()).unwrap();
                Ok(())
            },
        }
    }
}

pub trait AnilistTokenChecker {
    fn verify_token(&self, token: String) -> impl Future<Output = Result<bool, Box<dyn Error>>> + Send;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error;

    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;

    #[derive(Default, Clone)]
    struct MockStorage {
        secrets_stored: HashMap<String, String>,
    }

    impl SecretStorage for MockStorage {
        fn save_secret<T: Into<String>>(&mut self, name: T, value: T) -> Result<(), Box<dyn Error>> {
            self.secrets_stored.insert(name.into(), value.into());
            Ok(())
        }

        fn get_secret<T: Into<String>>(&self, secret_name: T) -> Result<Option<String>, Box<dyn Error>> {
            Ok(self.secrets_stored.get(&secret_name.into()).cloned())
        }

        fn remove_secret<T: AsRef<str>>(&mut self, secret_name: T) -> Result<(), Box<dyn Error>> {
            match self.secrets_stored.remove(secret_name.as_ref()) {
                Some(_val) => Ok(()),
                None => Err("secret did not exist".into()),
            }
        }
    }

    #[test]
    fn it_saves_anilist_access_token_and_user_id() {
        let cli = CliArgs::new();
        let acess_token = Uuid::new_v4().to_string();
        let user_id = "120398".to_string();

        let mut storage = MockStorage::default();

        cli.save_anilist_credentials(
            AnilistCredentialsProvided {
                access_token: &acess_token,
                client_id: &user_id,
            },
            &mut storage,
        )
        .expect("should not fail");

        let (secret_name, token) = storage.secrets_stored.get_key_value("anilist_access_token").unwrap();

        assert_eq!("anilist_access_token", secret_name);
        assert_eq!(acess_token, *token);

        let (secret_name, value) = storage.secrets_stored.get_key_value("anilist_client_id").unwrap();

        assert_eq!("anilist_client_id", secret_name);
        assert_eq!(user_id.parse::<u32>().unwrap(), value.parse::<u32>().unwrap());
    }

    #[derive(Debug)]
    struct AnilistCheckerTest {
        should_fail: bool,
        invalid_token: bool,
    }

    impl AnilistCheckerTest {
        fn succesful() -> Self {
            Self {
                should_fail: false,
                invalid_token: false,
            }
        }

        fn failing() -> Self {
            Self {
                should_fail: true,
                invalid_token: true,
            }
        }
    }
    impl AnilistTokenChecker for AnilistCheckerTest {
        async fn verify_token(&self, _token: String) -> Result<bool, Box<dyn Error>> {
            if self.invalid_token {
                return Ok(false);
            }

            Ok(true)
        }
    }

    #[tokio::test]
    async fn it_checks_acess_token_is_valid() -> Result<(), Box<dyn Error>> {
        let cli = CliArgs::new();

        let anilist_checker = AnilistCheckerTest::succesful();

        let token_is_valid = cli.check_anilist_token(&anilist_checker, "some_token".to_string()).await?;

        assert!(token_is_valid);

        let anilist_checker = AnilistCheckerTest::failing();

        let token_is_valid = cli.check_anilist_token(&anilist_checker, "some_token".to_string()).await?;

        assert!(!token_is_valid);
        Ok(())
    }
}
