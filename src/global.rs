use once_cell::sync::{OnceCell};

use crate::backend::filter::Languages;

pub static PREFERRED_LANGUAGE: OnceCell<Languages> = OnceCell::new();
