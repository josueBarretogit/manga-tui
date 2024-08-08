use once_cell::sync::{Lazy, OnceCell};
use ratatui::style::{Style, Stylize};

use crate::backend::filter::Languages;

pub static PREFERRED_LANGUAGE: OnceCell<Languages> = OnceCell::new();

pub static INSTRUCTIONS_STYLE: Lazy<Style> =
    Lazy::new(|| Style::default().bold().underlined().yellow());

pub static ERROR_STYLE: Lazy<Style> =
    Lazy::new(|| Style::default().bold().underlined().red().on_black());

pub static CURRENT_LIST_ITEM_STYLE: Lazy<Style> = Lazy::new(|| Style::default().on_blue());
