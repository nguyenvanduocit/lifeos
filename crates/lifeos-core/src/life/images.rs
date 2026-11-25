//! The images the owner puts in a message to an agent (remit's ADR-0111). The page puts an image's bytes first, by an
//! id it makes; the draft names it (`Draft::images`), the prompt it becomes keeps the names in its data, and the agent
//! reads the bytes with the words. The bytes live in the database beside the vault, never in its documents, and are
//! kept: they are the owner's.

use anyhow::{Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{OptionalExtension, params};
use std::path::PathBuf;

use super::Life;
use super::cards::now;
use lifeos_wire::{IMAGE_MAX_BYTES, Image, ImageRef, ImageType};

/// The path relative to the workspace, where an agent can read the original bytes.
pub fn attachment_path(id: &str) -> Result<PathBuf> {
    let Some((key, name)) = id.split_once('/') else { bail!("invalid file attachment id") };
    if key.len() != 32 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid file attachment id");
    }
    if name.is_empty() || name.len() > 1024 || matches!(name, "." | "..")
        || name.contains(['/', '\\']) || name.chars().any(char::is_control)
    {
        bail!("invalid attachment filename");
    }
    // The prefix prevents a file named CLAUDE.md or a dotfile from becoming workspace configuration.
    let stored_name = format!("file-{}", &name[..name.floor_char_boundary(250)]);
    Ok(PathBuf::from("attachments").join(key).join(stored_name))
}

impl Life {
    /// Keeps image as id. An id put again keeps what it was first put with, so a replayed call changes nothing.
    pub fn put_image(&self, id: &str, image: &Image) -> Result<()> {
        if image.media_type == ImageType::File {
            bail!("use the file attachment endpoint");
        }
        if id.trim().is_empty() {
            bail!("an image needs an id");
        }
        let bytes = match STANDARD.decode(image.data.as_bytes()) {
            Ok(bytes) => bytes,
            Err(err) => bail!("image {id} is not base64: {err}"),
        };
        if bytes.is_empty() {
            bail!("image {id} is empty");
        }
        if bytes.len() > IMAGE_MAX_BYTES {
            bail!("image {id} is {} bytes; a model reads one of at most {IMAGE_MAX_BYTES}", bytes.len());
        }
        self.conn.execute(
            "INSERT INTO images(id, media_type, bytes, at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(id) DO NOTHING",
            params![id, image.media_type.as_str(), bytes, now()],
        )?;
        Ok(())
    }

    /// Image id's kind and bytes.
    pub fn image(&self, id: &str) -> Result<Image> {
        let found = self
            .conn
            .query_row("SELECT media_type, bytes FROM images WHERE id = ?1", [id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .optional()?;
        let Some((media_type, bytes)) = found else { bail!("image {id} was not put") };
        Ok(Image { media_type: ImageType::parse(&media_type).unwrap_or_default(), data: STANDARD.encode(bytes) })
    }

    /// Refuses images naming one that was not put.
    pub fn check_images(&self, images: &[ImageRef]) -> Result<()> {
        for i in images {
            if i.media_type == ImageType::File {
                if !self.home().join(attachment_path(&i.id)?).is_file() {
                    bail!("file {} was not put", i.id);
                }
                continue;
            }
            let put: bool =
                self.conn.query_row("SELECT EXISTS(SELECT 1 FROM images WHERE id = ?1)", [&i.id], |r| r.get(0))?;
            if !put {
                bail!("image {} was not put", i.id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn life() -> (tempfile::TempDir, Life) {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        (dir, life)
    }

    fn png(data: &[u8]) -> Image {
        Image { media_type: ImageType::Png, data: STANDARD.encode(data) }
    }

    #[test]
    fn an_image_put_twice_keeps_its_first_bytes() {
        let (_dir, life) = life();
        life.put_image("im_1", &png(b"first")).unwrap();
        life.put_image("im_1", &Image { media_type: ImageType::Webp, data: STANDARD.encode(b"second") }).unwrap();
        assert_eq!(life.image("im_1").unwrap(), png(b"first"));
        assert!(life.image("im_2").is_err());
    }

    #[test]
    fn an_image_over_five_mib_or_not_base64_is_refused() {
        let (_dir, life) = life();
        let big = vec![7u8; IMAGE_MAX_BYTES + 1];
        let err = life.put_image("im_big", &png(&big)).unwrap_err().to_string();
        assert!(err.contains("at most"), "{err}");
        assert!(life.image("im_big").is_err());
        life.put_image("im_max", &png(&big[..IMAGE_MAX_BYTES])).unwrap();
        assert!(life.put_image("im_bad", &Image { media_type: ImageType::Png, data: "not base64!".into() }).is_err());
        assert!(life.put_image("im_empty", &png(b"")).is_err());
        assert!(life.put_image("", &png(b"x")).is_err());
    }

    #[test]
    fn a_draft_names_only_images_that_were_put() {
        let (_dir, life) = life();
        let shot = ImageRef { id: "im_1".into(), media_type: ImageType::Png };
        assert!(life.set_draft("assistant", "xem", std::slice::from_ref(&shot)).is_err());
        life.put_image("im_1", &png(b"x")).unwrap();
        let d = life.set_draft("assistant", "xem", std::slice::from_ref(&shot)).unwrap();
        assert_eq!((d.text.as_str(), d.images.as_slice()), ("xem", std::slice::from_ref(&shot)));
        assert_eq!(life.draft("assistant").unwrap().images, [shot]);
    }
}
