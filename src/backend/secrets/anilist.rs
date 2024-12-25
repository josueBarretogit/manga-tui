use std::error::Error;

use clap::crate_name;
use keyring::Entry;
use strum::Display;

use super::SecretStorage;

#[derive(Debug)]
pub struct AnilistStorage {
    service_name: &'static str,
}

#[derive(Debug, Display, Clone, Copy)]
pub enum AnilistCredentials {
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

#[derive(Debug, Clone)]
pub struct Credentials {
    pub access_token: String,
    pub client_id: String,
}

impl AnilistStorage {
    pub fn new() -> Self {
        Self {
            service_name: crate_name!(),
        }
    }

    pub fn check_credentials_stored(&self) -> Result<Option<Credentials>, Box<dyn Error>> {
        let credentials = self.get_multiple_secrets([AnilistCredentials::ClientId, AnilistCredentials::AccessToken].into_iter())?;

        let client_id = credentials.get(&AnilistCredentials::ClientId.to_string()).cloned();
        let access_token = credentials.get(&AnilistCredentials::AccessToken.to_string()).cloned();

        match client_id.zip(access_token) {
            Some((id, token)) => {
                if id.is_empty() || token.is_empty() {
                    return Ok(None);
                }

                Ok(Some(Credentials {
                    access_token: token,
                    client_id: id.parse().unwrap(),
                }))
            },
            None => Ok(None),
        }
    }
}

impl SecretStorage for AnilistStorage {
    fn save_secret<T: Into<String>>(&mut self, secret_name: T, value: T) -> Result<(), Box<dyn std::error::Error>> {
        let secret = Entry::new(self.service_name, &secret_name.into())?;

        let secret_as_string: String = value.into();

        secret.set_secret(secret_as_string.as_bytes())?;

        Ok(())
    }

    fn get_secret<T: Into<String>>(&self, secret_name: T) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let secret = Entry::new(self.service_name, &secret_name.into())?;

        match secret.get_secret() {
            Ok(secret_as_bytes) => Ok(Some(String::from_utf8(secret_as_bytes)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn remove_secret<T: AsRef<str>>(&mut self, secret_name: T) -> Result<(), Box<dyn std::error::Error>> {
        let secret = Entry::new(self.service_name, secret_name.as_ref())?;

        secret.delete_credential()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //use std::collections::HashMap;
    //use std::error::Error;
    //
    //use super::*;
    //
    // commented out because dont know how to mock keyring's functionality itself

    //#[test]
    //fn it_stores_anilist_account_secrets() -> Result<(), Box<dyn Error>> {
    //    let id = "some_string".to_string();
    //    let code = "some_string".to_string();
    //    let secret = "some_string".to_string();
    //
    //    let mut anilist_storage = AnilistStorage::new();
    //
    //    anilist_storage.save_multiple_secrets(HashMap::from([
    //        ("id".to_string(), id.clone()),
    //        ("code".to_string(), code.clone()),
    //        ("secret".to_string(), secret.clone()),
    //    ]))?;
    //
    //    let id_stored = anilist_storage.get_secret("id")?.unwrap();
    //    assert_eq!(id_stored, id);
    //
    //    let code_stored = anilist_storage.get_secret("code")?.unwrap();
    //    assert_eq!(code_stored, code);
    //
    //    let secret_stored = anilist_storage.get_secret("secret")?.unwrap();
    //    assert_eq!(secret_stored, secret);
    //
    //    Ok(())
    //}

    //#[test]
    //fn it_retrieves_anilist_credential() -> Result<(), Box<dyn Error>> {
    //    set_default_credential_builder(mock::default_credential_builder());
    //    let anilist_storage = AnilistStorage::new();
    //
    //    let should_be_empty = anilist_storage.check_credentials_stored()?;
    //
    //    assert!(should_be_empty.is_none());
    //
    //    Ok(())
    //}
}
