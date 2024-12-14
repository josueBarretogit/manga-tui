use std::collections::HashMap;
use std::error::Error;
use std::io::BufRead;

use clap::{crate_version, Parser, Subcommand};
use strum::{Display, IntoEnumIterator};

use crate::backend::filter::Languages;
use crate::backend::secrets::SecretStorage;
use crate::backend::APP_DATA_DIR;
use crate::global::PREFERRED_LANGUAGE;
use crate::logger::ILogger;

fn read_input(mut input_reader: impl BufRead, logger: &impl ILogger, message: &str) -> Result<String, Box<dyn Error>> {
    logger.inform(message);
    let mut input_provided = String::new();
    input_reader.read_line(&mut input_provided)?;
    Ok(input_provided)
}

#[derive(Debug)]
pub enum AnilistStatus {
    Setup,
    MissigCredentials,
}

#[derive(Subcommand)]
pub enum AnilistCommand {
    /// setup anilist client to be able to sync reading progress
    Init,
    /// check wheter or not anilist is setup correctly
    Status,
}

#[derive(Subcommand)]
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

#[derive(Debug, Display)]
enum AnilistCredentials {
    #[strum(to_string = "anilist_client_id")]
    ClientId,
    #[strum(to_string = "anilist_secret")]
    Secret,
    #[strum(to_string = "anilist_code")]
    Code,
    #[strum(to_string = "anilist_access_token")]
    AccessToken,
}

impl From<AnilistCredentials> for String {
    fn from(value: AnilistCredentials) -> Self {
        value.to_string()
    }
}

#[derive(Parser)]
#[command(version = crate_version!())]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long)]
    pub data_dir: bool,
}

pub struct AnilistCredentialsProvided<'a> {
    pub code: &'a str,
    pub secret: &'a str,
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

    fn save_anilist_credentials(
        &self,
        credentials: AnilistCredentialsProvided<'_>,
        storage: &mut impl SecretStorage,
    ) -> Result<(), Box<dyn Error>> {
        storage.save_multiple_secrets(HashMap::from([
            (AnilistCredentials::ClientId.to_string(), credentials.client_id.to_string()),
            (AnilistCredentials::Code.to_string(), credentials.code.to_string()),
            (AnilistCredentials::Secret.to_string(), credentials.secret.to_string()),
        ]))?;

        Ok(())
    }

    pub fn init_anilist(
        self,
        mut input_reader: impl BufRead,
        storage: &mut impl SecretStorage,
        logger: impl ILogger,
    ) -> Result<(), Box<dyn Error>> {
        let client_id = read_input(&mut input_reader, &logger, "Provide the client id")?;
        let secret = read_input(&mut input_reader, &logger, "Provide the secret")?;
        let code = read_input(&mut input_reader, &logger, "Provide the code")?;

        self.save_anilist_credentials(
            AnilistCredentialsProvided {
                code: &code,
                secret: &secret,
                client_id: &client_id,
            },
            storage,
        )?;

        logger.inform("Anilist was correctly setup :D");

        Ok(())
    }

    fn anilist_status(&self, storage: &impl SecretStorage) -> Result<AnilistStatus, Box<dyn Error>> {
        let credentials = [
            storage.get_secret(AnilistCredentials::Code)?,
            storage.get_secret(AnilistCredentials::Secret)?,
            storage.get_secret(AnilistCredentials::ClientId)?,
        ];

        for credential in credentials {
            if credential.is_none() {
                return Ok(AnilistStatus::MissigCredentials);
            }
        }

        Ok(AnilistStatus::Setup)
    }

    pub fn proccess_args(self) -> Result<(), Box<dyn Error>> {
        if self.data_dir {
            let app_dir = APP_DATA_DIR.as_ref().unwrap();
            println!("{}", app_dir.to_str().unwrap());
            return Ok(());
        }

        match self.command {
            Some(command) => match command {
                Commands::Lang { print, set } => {
                    if print {
                        Self::print_available_languages();
                        std::process::exit(1)
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

                                std::process::exit(1)
                            }

                            PREFERRED_LANGUAGE.set(try_lang.unwrap()).unwrap();
                        },
                        None => {
                            PREFERRED_LANGUAGE.set(Languages::default()).unwrap();
                        },
                    }
                    Ok(())
                },

                Commands::Anilist { command } => todo!(),
            },
            None => {
                PREFERRED_LANGUAGE.set(Languages::default()).unwrap();
                Ok(())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error;

    use pretty_assertions::assert_eq;
    use uuid::Uuid;
    use Commands::*;

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

        fn save_multiple_secrets<T: Into<String>>(&mut self, values: HashMap<T, T>) -> Result<(), Box<dyn Error>> {
            for (key, name) in values {
                self.save_secret(key, name)?;
            }
            Ok(())
        }

        fn get_secret<T: Into<String>>(&self, secret_name: T) -> Result<Option<String>, Box<dyn Error>> {
            Ok(self.secrets_stored.get(&secret_name.into()).cloned())
        }
    }

    #[test]
    fn it_saves_anilist_account_credentials() {
        let cli = CliArgs::new();
        let client_id_provided = Uuid::new_v4().to_string();
        let secret_provided = Uuid::new_v4().to_string();
        let code_provided = Uuid::new_v4().to_string();

        let mut storage = MockStorage::default();

        cli.save_anilist_credentials(
            AnilistCredentialsProvided {
                code: &code_provided,
                secret: &secret_provided,
                client_id: &client_id_provided,
            },
            &mut storage,
        )
        .expect("should not panic");

        let (name, id) = storage.secrets_stored.get_key_value("anilist_client_id").unwrap();
        let (key_name2, secret) = storage.secrets_stored.get_key_value("anilist_secret").unwrap();
        let (key_name3, code) = storage.secrets_stored.get_key_value("anilist_code").unwrap();

        assert_eq!("anilist_client_id", name);
        assert_eq!(client_id_provided, *id);

        assert_eq!("anilist_secret", key_name2);
        assert_eq!(secret_provided, *secret);

        assert_eq!("anilist_code", key_name3);
        assert_eq!(code_provided, *code);
    }

    #[test]
    fn it_checks_anilist_is_setup() {
        let cli = CliArgs::new();

        let mut storage = MockStorage::default();

        let not_setup = cli.anilist_status(&storage).unwrap();

        assert!(!matches!(not_setup, AnilistStatus::Setup));

        let client_id_provided = Uuid::new_v4().to_string();
        let secret_provided = Uuid::new_v4().to_string();
        let code_provided = Uuid::new_v4().to_string();

        // after storing the credentials it should have a ok status
        cli.save_anilist_credentials(
            AnilistCredentialsProvided {
                code: &code_provided,
                secret: &secret_provided,
                client_id: &client_id_provided,
            },
            &mut storage,
        )
        .expect("should not panic");

        let is_setup = cli.anilist_status(&storage).unwrap();

        assert!(matches!(is_setup, AnilistStatus::Setup));
    }
}
