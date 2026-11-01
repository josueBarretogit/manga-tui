use crate::config::{MangaTuiConfig, UiLanguage};

/// Returns `true` when the configured UI language is Spanish (`es`, default).
#[inline]
pub fn is_es() -> bool {
    MangaTuiConfig::get().ui_language == UiLanguage::Es
}

/// Selects between Spanish (`es`) and English (`en`) UI copy based on `config.toml`.
#[inline]
pub fn tr<'a>(es: &'a str, en: &'a str) -> &'a str {
    if is_es() { es } else { en }
}
