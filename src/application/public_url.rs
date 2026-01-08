//! Public URL builder.
//!
//! This module provides functionality for constructing public URLs
//! for `SeaweedFS` files, including support for image resize parameters.

use crate::domain::{DomainError, DomainResult, FileId, LookupResult, MasterPort};

/// Image resize mode for public URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    /// Resize to fit within dimensions, maintaining aspect ratio.
    Fit,
    /// Resize to fill dimensions, cropping if necessary.
    Fill,
}

/// Parameters for image transformation in URLs.
#[derive(Debug, Clone, Default)]
pub struct ImageParams {
    /// Target width in pixels.
    pub width: Option<u32>,
    /// Target height in pixels.
    pub height: Option<u32>,
    /// Resize mode.
    pub mode: Option<ResizeMode>,
}

impl ImageParams {
    /// Creates new image params with the specified width.
    #[must_use]
    pub const fn width(width: u32) -> Self {
        Self {
            width: Some(width),
            height: None,
            mode: None,
        }
    }

    /// Creates new image params with the specified height.
    #[must_use]
    pub const fn height(height: u32) -> Self {
        Self {
            width: None,
            height: Some(height),
            mode: None,
        }
    }

    /// Creates new image params with width and height.
    #[must_use]
    pub const fn dimensions(width: u32, height: u32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
            mode: None,
        }
    }

    /// Sets the resize mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: ResizeMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Renders the image params as a query string suffix.
    ///
    /// Returns an empty string if no params are set.
    #[must_use]
    pub fn render(&self) -> String {
        let mut params = Vec::new();

        if let Some(w) = self.width {
            params.push(format!("width={w}"));
        }
        if let Some(h) = self.height {
            params.push(format!("height={h}"));
        }
        if let Some(mode) = self.mode {
            let mode_str = match mode {
                ResizeMode::Fit => "fit",
                ResizeMode::Fill => "fill",
            };
            params.push(format!("mode={mode_str}"));
        }

        if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        }
    }
}

/// Options for building public URLs.
#[derive(Debug, Clone, Default)]
pub struct PublicUrlOptions {
    /// Image transformation parameters.
    pub image_params: Option<ImageParams>,
    /// Prefer public URL over internal URL.
    pub prefer_public: bool,
}

/// Use case for building public URLs for `SeaweedFS` files.
pub struct PublicUrlBuilder<M> {
    master: M,
}

impl<M> PublicUrlBuilder<M>
where
    M: MasterPort,
{
    /// Creates a new `PublicUrlBuilder`.
    pub const fn new(master: M) -> Self {
        Self { master }
    }

    /// Builds a public URL for the given file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume lookup fails or no replicas are available.
    pub async fn build(
        &self,
        file_id: &FileId,
        options: Option<PublicUrlOptions>,
    ) -> DomainResult<String> {
        let opts = options.unwrap_or_default();

        let lookup = self.master.lookup(file_id.volume_id()).await?;

        if lookup.locations.is_empty() {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let location = &lookup.locations[0];

        let base_url = if opts.prefer_public {
            location
                .public_url
                .as_ref()
                .unwrap_or(&location.url)
                .clone()
        } else {
            location.url.clone()
        };

        let fid_str = file_id.render();
        let image_suffix = opts
            .image_params
            .as_ref()
            .map(ImageParams::render)
            .unwrap_or_default();

        Ok(format!("{base_url}/{fid_str}{image_suffix}"))
    }

    /// Builds a URL from a pre-fetched lookup result.
    ///
    /// # Errors
    ///
    /// Returns an error if no replicas are available in the lookup result.
    pub fn build_from_lookup(
        file_id: &FileId,
        lookup: &LookupResult,
        options: Option<PublicUrlOptions>,
    ) -> DomainResult<String> {
        let opts = options.unwrap_or_default();

        if lookup.locations.is_empty() {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let location = &lookup.locations[0];
        let base_url = if opts.prefer_public {
            location
                .public_url
                .as_ref()
                .unwrap_or(&location.url)
                .clone()
        } else {
            location.url.clone()
        };

        let fid_str = file_id.render();
        let image_suffix = opts
            .image_params
            .as_ref()
            .map(ImageParams::render)
            .unwrap_or_default();

        Ok(format!("{base_url}/{fid_str}{image_suffix}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_params_width() {
        let params = ImageParams::width(100);
        assert_eq!(params.width, Some(100));
        assert_eq!(params.height, None);
    }

    #[test]
    fn test_image_params_dimensions() {
        let params = ImageParams::dimensions(100, 200);
        assert_eq!(params.width, Some(100));
        assert_eq!(params.height, Some(200));
    }

    #[test]
    fn test_image_params_with_mode() {
        let params = ImageParams::dimensions(100, 200).with_mode(ResizeMode::Fill);
        assert_eq!(params.mode, Some(ResizeMode::Fill));
    }

    #[test]
    fn test_image_params_render_empty() {
        let params = ImageParams::default();
        assert_eq!(params.render(), "");
    }

    #[test]
    fn test_image_params_render_width_only() {
        let params = ImageParams::width(100);
        assert_eq!(params.render(), "?width=100");
    }

    #[test]
    fn test_image_params_render_full() {
        let params = ImageParams::dimensions(100, 200).with_mode(ResizeMode::Fit);
        let rendered = params.render();
        assert!(rendered.contains("width=100"));
        assert!(rendered.contains("height=200"));
        assert!(rendered.contains("mode=fit"));
    }
}
