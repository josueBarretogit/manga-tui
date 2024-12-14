pub mod anilist;

use std::collections::HashMap;
use std::error::Error;

/// Abstraction of the location where secrets will be stored
pub trait SecretStorage {
    fn save_secret<T: Into<String>>(&mut self, secret_name: T, value: T) -> Result<(), Box<dyn Error>>;
    fn save_multiple_secrets<T: Into<String>>(&mut self, values: HashMap<T, T>) -> Result<(), Box<dyn Error>>;
    fn get_secret<T: Into<String>>(&self, _secret_name: T) -> Result<Option<String>, Box<dyn Error>> {
        Err("not implemented".into())
    }
}
