//! Upload checks for card photos.
//!
//! The client resizes and re-encodes photos before uploading, so anything
//! large is suspicious. The declared type must match the file's magic bytes;
//! responses are served with that type and `nosniff`.

use crate::error::ApiError;

/// Client-side re-encoding targets ~200 KB; leave headroom for PNG fallbacks.
pub const MAX_IMAGE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMime {
    Jpeg,
    Png,
    Webp,
}

impl ImageMime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    fn from_content_type(value: &str) -> Option<Self> {
        let essence = value.split(';').next().unwrap_or_default().trim();
        match essence.to_ascii_lowercase().as_str() {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    fn matches(self, data: &[u8]) -> bool {
        match self {
            Self::Jpeg => data.starts_with(&[0xff, 0xd8, 0xff]),
            Self::Png => data.starts_with(b"\x89PNG\r\n\x1a\n"),
            Self::Webp => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        }
    }
}

/// Validates an upload: supported type, matching content, sensible size.
pub fn check_upload(content_type: Option<&str>, data: &[u8]) -> Result<ImageMime, ApiError> {
    let mime = content_type
        .and_then(ImageMime::from_content_type)
        .ok_or_else(|| ApiError::Validation("photos must be JPEG, PNG or WebP".into()))?;
    if data.len() > MAX_IMAGE_BYTES {
        return Err(ApiError::Validation(
            "photo is too large (max 512 KB)".into(),
        ));
    }
    if !mime.matches(data) {
        return Err(ApiError::Validation(
            "photo content does not match its type".into(),
        ));
    }
    Ok(mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 0x10];
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const WEBP: &[u8] = b"RIFF\x24\0\0\0WEBPVP8 ";

    #[test]
    fn accepts_supported_types_with_matching_content() {
        assert_eq!(
            check_upload(Some("image/jpeg"), JPEG).unwrap(),
            ImageMime::Jpeg
        );
        assert_eq!(
            check_upload(Some("image/png"), PNG).unwrap(),
            ImageMime::Png
        );
        assert_eq!(
            check_upload(Some("Image/WebP; q=1"), WEBP).unwrap(),
            ImageMime::Webp
        );
    }

    #[test]
    fn rejects_missing_or_unsupported_types() {
        assert!(check_upload(None, JPEG).is_err());
        assert!(check_upload(Some("image/svg+xml"), b"<svg/>").is_err());
        assert!(check_upload(Some("text/html"), JPEG).is_err());
    }

    #[test]
    fn rejects_content_that_does_not_match_the_declared_type() {
        let error = check_upload(Some("image/png"), JPEG).unwrap_err();
        assert!(error.to_string().contains("does not match"));
        assert!(check_upload(Some("image/webp"), b"RIFF").is_err());
        assert!(check_upload(Some("image/jpeg"), b"").is_err());
    }

    #[test]
    fn rejects_oversized_uploads() {
        let mut big = JPEG.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(
            check_upload(Some("image/jpeg"), &big)
                .unwrap_err()
                .to_string()
                .contains("too large")
        );
    }
}
