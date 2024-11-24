use std::collections::HashMap;
use std::error::Error;
use std::io::{self, BufRead};

use clap::{crate_version, Parser, Subcommand};
use strum::IntoEnumIterator;

use crate::backend::filter::Languages;
use crate::backend::APP_DATA_DIR;
use crate::global::PREFERRED_LANGUAGE;

fn read_input(mut input_reader: impl BufRead, message: &str) -> Result<String, Box<dyn Error>> {
    println!("{message}");
    let mut input_provided = String::new();
    input_reader.read_line(&mut input_provided)?;
    Ok(input_provided)
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
        #[arg(short, long)]
        init: bool,
    },
}

#[derive(Parser)]
#[command(version = crate_version!())]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long)]
    pub data_dir: bool,
}

/// Abstraction of the location where secrets will be stored
pub trait SecretStorage {
    fn save_secret(&mut self, secret_name: String, value: String) -> Result<(), Box<dyn Error>>;
    fn save_multiple_secrets(&mut self, values: HashMap<String, String>) -> Result<(), Box<dyn Error>>;
    fn get_secret(&self, _secret_name: &str) -> Result<String, Box<dyn Error>> {
        Err("not implemented".into())
    }
}

struct AnilistCredentialsProvided<'a> {
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
        storage: &mut dyn SecretStorage,
    ) -> Result<(), Box<dyn Error>> {
        storage.save_multiple_secrets(HashMap::from([
            ("anilist_client_id".to_string(), credentials.client_id.to_string()),
            ("anilist_code".to_string(), credentials.code.to_string()),
            ("anilist_secret".to_string(), credentials.secret.to_string()),
        ]))?;

        Ok(())
    }

    pub fn init_anilist(self, mut input_reader: impl BufRead, storage: &mut dyn SecretStorage) -> Result<(), Box<dyn Error>> {
        let client_id = read_input(&mut input_reader, "Provide the client id")?;
        let secret = read_input(&mut input_reader, "Provide the secret")?;
        let code = read_input(&mut input_reader, "Provide the code")?;

        self.save_anilist_credentials(
            AnilistCredentialsProvided {
                code: &code,
                secret: &secret,
                client_id: &client_id,
            },
            storage,
        )?;

        println!("Anilist was correctly setup :D");

        Ok(())
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
                        return Ok(());
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

                                return Ok(());
                            }

                            PREFERRED_LANGUAGE.set(try_lang.unwrap()).unwrap();
                        },
                        None => {
                            PREFERRED_LANGUAGE.set(Languages::default()).unwrap();
                        },
                    }
                    Ok(())
                },

                Commands::Anilist { init } => todo!(),
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
    use std::io::{BufReader, Cursor};

    use pretty_assertions::assert_eq;
    use uuid::Uuid;
    use Commands::*;

    use super::*;

    #[derive(Default, Clone)]
    struct MockStorage {
        secrets_stored: HashMap<String, String>,
    }

    impl SecretStorage for MockStorage {
        fn save_secret(&mut self, name: String, value: String) -> Result<(), Box<dyn Error>> {
            self.secrets_stored.insert(name, value);
            Ok(())
        }

        fn save_multiple_secrets(&mut self, values: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
            for (key, name) in values {
                self.save_secret(key, name)?;
            }
            Ok(())
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
}
