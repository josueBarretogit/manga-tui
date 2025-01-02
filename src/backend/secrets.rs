pub mod anilist;

use std::collections::HashMap;
use std::error::Error;

/// Abstraction of the location where secrets will be stored
pub trait SecretStorage {
    fn save_secret<T: Into<String>>(&mut self, secret_name: T, value: T) -> Result<(), Box<dyn Error>>;

    fn save_multiple_secrets<T: Into<String>>(&mut self, values: HashMap<T, T>) -> Result<(), Box<dyn Error>> {
        for (name, value) in values {
            self.save_secret(name, value)?
        }
        Ok(())
    }

    fn remove_multiple_secrets<T: AsRef<str>>(&mut self, values: impl Iterator<Item = T>) -> Result<(), Box<dyn Error>> {
        for name in values {
            self.remove_secret(name)?
        }
        Ok(())
    }

    fn get_multiple_secrets<T: Into<String>>(
        &self,
        secrets_names: impl Iterator<Item = T>,
    ) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let mut secrets_collected: HashMap<String, String> = HashMap::new();

        for secrets in secrets_names {
            let secret_to_find: String = secrets.into();
            if let Some(secret) = self.get_secret(secret_to_find.clone())? {
                secrets_collected.insert(secret_to_find, secret);
            }
        }

        Ok(secrets_collected)
    }

    fn get_secret<T: Into<String>>(&self, _secret_name: T) -> Result<Option<String>, Box<dyn Error>>;

    fn remove_secret<T: AsRef<str>>(&mut self, secret_name: T) -> Result<(), Box<dyn Error>>;
}
