#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Latte,
    Frappe,
    Macchiato,
    Mocha,
}

use crate::config::lua::deserializer::string_enum;

string_enum!(
    Flavor,
    "a Catppuccin flavour",
    "latte" => Flavor::Latte,
    "frappe" => Flavor::Frappe,
    "macchiato" => Flavor::Macchiato,
    "mocha" => Flavor::Mocha,
);
