//! lifeos core: the vault (Markdown), the database (SQLite) and the tools
//! every interface shares.

/// A tool's input: one struct that is the tool's JSON schema (schemars), its
/// parser (serde), its TypeScript (ts-rs, `make gen`) and the argument of the
/// `Life` method that does the work.
macro_rules! input {
    ($(#[$m:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
        pub struct $name { $($body)* }
    };
}

/// A closed set of words a tool input takes.
macro_rules! choice {
    ($(#[$m:meta])* pub enum $name:ident { $($(#[$vm:meta])* $variant:ident => $word:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
        #[serde(rename_all = "lowercase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename_all = "lowercase"))]
        pub enum $name { $($(#[$vm])* $variant,)* }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $word,)* }
            }
        }
    };
}

pub mod db;
pub mod life;
pub mod tools;
pub mod vault;

use std::path::PathBuf;

pub use life::Life;

/// `$LIFEOS_HOME`, else `~/lifeos`.
pub fn default_home() -> PathBuf {
    std::env::var_os("LIFEOS_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join("lifeos")))
        .unwrap_or_else(|| PathBuf::from("lifeos"))
}
