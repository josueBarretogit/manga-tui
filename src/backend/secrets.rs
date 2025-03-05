pub mod keyring;

use std::collections::HashMap;
use std::error::Error;

/// Abstraction of the service which stores sensitive data, or secrets
pub trait SecretStorage {
    /// Store the secret which will be identified by the `secret_name` and the secret itself is the
    /// `value`
    fn save_secret<T: Into<String>, S: Into<String>>(&self, secret_name: T, value: S) -> Result<(), Box<dyn Error>>;

    fn get_secret<T: Into<String>>(&self, secret_name: T) -> Result<Option<String>, Box<dyn Error>>;

    fn remove_secret<T: AsRef<str>>(&self, secret_name: T) -> Result<(), Box<dyn Error>>;

    fn save_multiple_secrets<T: Into<String>, S: Into<String>>(&self, values: HashMap<T, S>) -> Result<(), Box<dyn Error>> {
        for (name, value) in values {
            self.save_secret(name, value)?
        }
        Ok(())
    }

    fn remove_multiple_secrets<T: AsRef<str>>(&self, values: impl Iterator<Item = T>) -> Result<(), Box<dyn Error>> {
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
}
