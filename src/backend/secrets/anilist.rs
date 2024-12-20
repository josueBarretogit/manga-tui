use clap::crate_name;
use keyring::Entry;

use super::SecretStorage;

#[derive(Debug)]
pub struct AnilistStorage {
    service_name: &'static str,
}

impl AnilistStorage {
    pub fn new() -> Self {
        Self {
            service_name: crate_name!(),
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
    use std::collections::HashMap;
    use std::error::Error;

    use uuid::Uuid;

    use super::*;

    #[test]
    fn it_stores_anilist_account_secrets() -> Result<(), Box<dyn Error>> {
        let id = Uuid::new_v4().to_string();
        let code = Uuid::new_v4().to_string();
        let secret = Uuid::new_v4().to_string();

        let mut anilist_storage = AnilistStorage::new();

        anilist_storage
            .save_multiple_secrets(HashMap::from([
                ("id".to_string(), id.clone()),
                ("code".to_string(), code.clone()),
                ("secret".to_string(), secret.clone()),
            ]))
            .unwrap();

        let id_stored = anilist_storage.get_secret("id")?.unwrap();
        assert_eq!(id_stored, id);

        let code_stored = anilist_storage.get_secret("code")?.unwrap();
        assert_eq!(code_stored, code);

        let secret_stored = anilist_storage.get_secret("secret")?.unwrap();
        assert_eq!(secret_stored, secret);

        Ok(())
    }
}
