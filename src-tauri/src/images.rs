//! Pictures as the window can show them. Most go through as they are; a JPEG XL picture is decoded and handed over as
//! PNG, as the web view (WebKitGTK on Linux, for one) has no decoder for it. A very large decoded picture is scaled down
//! to `MAX_SIDE` for display: the file itself is never changed. Decoding takes a while, so the last few decoded pictures
//! are kept in memory (`CACHE_BYTES`), each by its file, size and time of change: a picture shown again comes at once.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::error::{Error, Result};

/// The longest side a decoded picture is shown at.
const MAX_SIDE: u32 = 2400;

/// The largest file read for a preview.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;

/// How much of decoded pictures is kept in memory, at most.
const CACHE_BYTES: usize = 96 * 1024 * 1024;

type Key = (PathBuf, u64, Option<SystemTime>);
static DECODED: Mutex<VecDeque<(Key, Arc<Vec<u8>>)>> = Mutex::new(VecDeque::new());

/// The picture in `file` as the window can show it, decoded once for as long as the file stays as it is.
pub fn displayable_file(file: &Path) -> Result<Vec<u8>> {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !is_jxl(name) {
        return std::fs::read(file).map_err(|e| Error::io(file, e));
    }
    let meta = std::fs::metadata(file).map_err(|e| Error::io(file, e))?;
    let key: Key = (file.to_path_buf(), meta.len(), meta.modified().ok());
    if let Some((_, png)) = DECODED.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return Ok(png.as_ref().clone());
    }
    let png = Arc::new(displayable(name, std::fs::read(file).map_err(|e| Error::io(file, e))?)?);
    let mut cache = DECODED.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(k, _)| k.0 != key.0);
    cache.push_back((key, png.clone()));
    while cache.len() > 1 && cache.iter().map(|(_, b)| b.len()).sum::<usize>() > CACHE_BYTES {
        cache.pop_front();
    }
    Ok(png.as_ref().clone())
}

fn is_jxl(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("jxl"))
}

/// The bytes of the picture `name` as the window can show them: JPEG XL becomes PNG, anything else is left as it is.
pub fn displayable(name: &str, bytes: Vec<u8>) -> Result<Vec<u8>> {
    if !is_jxl(name) {
        return Ok(bytes);
    }
    let bad = |e: image::ImageError| Error::invalid(format!("This JPEG XL picture can't be read: {e}"));
    let decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(bytes)).map_err(bad)?;
    let mut picture = image::DynamicImage::from_decoder(decoder).map_err(bad)?;
    if picture.width().max(picture.height()) > MAX_SIDE {
        picture = picture.thumbnail(MAX_SIDE, MAX_SIDE);
    }
    // PNG takes 8 or 16 bits per channel; a picture decoded as floats is brought down to 8.
    if matches!(picture, image::DynamicImage::ImageRgb32F(_) | image::DynamicImage::ImageRgba32F(_)) {
        picture = image::DynamicImage::ImageRgba8(picture.to_rgba8());
    }
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).map_err(bad)?;
    Ok(out.into_inner())
}

/// A picture somewhere on this machine (a file being dropped onto the window), for its preview: pictures only, and not
/// larger than `MAX_BYTES`.
pub fn preview_of(path: &Path) -> Result<Vec<u8>> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !crate::uploads::is_image(name) {
        return Err(Error::invalid("Only pictures have a preview"));
    }
    let size = std::fs::metadata(path).map_err(|e| Error::io(path, e))?.len();
    if size > MAX_BYTES {
        return Err(Error::invalid("This picture is too large to preview"));
    }
    displayable_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_that_isnt_jpeg_xl_goes_through_as_it_is() {
        assert_eq!(displayable("a.png", vec![1, 2, 3]).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn a_broken_jpeg_xl_picture_is_said_not_to_be_readable() {
        assert!(displayable("a.jxl", b"not a picture".to_vec()).is_err());
    }

    #[test]
    fn a_file_that_isnt_a_picture_has_no_preview() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.pdf");
        std::fs::write(&file, b"%PDF").unwrap();
        assert!(preview_of(&file).is_err());
    }
}
