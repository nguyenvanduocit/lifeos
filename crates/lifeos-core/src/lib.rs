//! lifeos core: the vault (Markdown), the database (SQLite) and the tools
//! every interface shares.

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
