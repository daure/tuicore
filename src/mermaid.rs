use std::fmt;

use merman::render::{
    HeadlessRenderer, HostThemeAppearance, HostThemeOutput, HostThemeProfile, HostThemeRoles,
    raster::RasterOptions,
};
use ratatui::style::Color;

use crate::{Image, ImageError, Theme};

/// Merman's host-theme profile for advanced SVG and raster customization.
pub use merman::render::HostThemeProfile as MermaidHostTheme;
/// Bounding box used to set a raster output resolution.
pub use merman::render::raster::RasterFitBox as MermaidRasterFitBox;
/// Raster options used by [`MermaidRenderer::render_png_with`] and
/// [`MermaidRenderer::render_image_with`].
pub use merman::render::raster::RasterOptions as MermaidRasterOptions;

/// Build a Merman host theme from tuicore's semantic color roles.
pub fn mermaid_host_theme(theme: &Theme) -> MermaidHostTheme {
    let canvas = css_color(theme.background_bg());
    let surface = css_color(theme.surface_bg());
    let surface_alt = css_color(theme.selected_bg());
    let text = css_color(theme.text_fg());
    let muted = css_color(theme.muted_fg());
    let border = css_color(theme.border_fg());
    let accent = css_color(theme.accent_fg());
    let success = css_color(theme.success_fg());
    let warning = css_color(theme.warning_fg());
    let error = css_color(theme.error_fg());
    let key = css_color(theme.key_fg());
    let appearance = if is_dark(theme.background_bg()) {
        HostThemeAppearance::Dark
    } else {
        HostThemeAppearance::Light
    };
    let series_palette = [
        accent.clone(),
        success.clone(),
        warning.clone(),
        key,
        error.clone(),
    ]
    .into_iter()
    .flatten();

    HostThemeProfile::builder()
        .appearance(appearance)
        .roles(HostThemeRoles {
            canvas: canvas.clone(),
            surface: surface.clone(),
            surface_alt: surface_alt.clone(),
            surface_muted: surface_alt.clone(),
            text: text.clone(),
            subtle_text: muted,
            border: border.clone(),
            line: border.clone(),
            edge_label_background: canvas.clone(),
            cluster_background: surface_alt,
            cluster_border: border.clone(),
            note_background: surface.clone(),
            note_border: border.clone(),
            note_text: text.clone(),
            actor_background: surface,
            actor_border: border,
            actor_text: text,
            activation_background: canvas,
            activation_border: accent,
            error,
            warning,
            success,
        })
        .series_palette(series_palette)
        .output(HostThemeOutput::resvg_safe_editor())
        .build()
}

/// A reusable, native Mermaid renderer for SVG and terminal-ready images.
///
/// The renderer does not require Node.js or a browser. Keep one instance for repeated renders.
pub struct MermaidRenderer {
    renderer: HeadlessRenderer,
}

impl Default for MermaidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidRenderer {
    /// Create a Mermaid renderer with Merman's deterministic defaults.
    pub fn new() -> Self {
        Self {
            renderer: HeadlessRenderer::new(),
        }
    }

    /// Render Mermaid source to SVG.
    pub fn render_svg(&self, source: impl AsRef<str>) -> Result<String, MermaidError> {
        self.renderer
            .render_svg_sync(source.as_ref())
            .map_err(MermaidError::Svg)?
            .ok_or(MermaidError::NoDiagram)
    }

    /// Render Mermaid source to SVG using a tuicore [`Theme`].
    pub fn render_svg_with_theme(
        &self,
        source: impl AsRef<str>,
        theme: &Theme,
    ) -> Result<String, MermaidError> {
        self.renderer
            .clone()
            .with_host_theme(&mermaid_host_theme(theme))
            .render_svg_sync(source.as_ref())
            .map_err(MermaidError::Svg)?
            .ok_or(MermaidError::NoDiagram)
    }

    /// Render Mermaid source to PNG bytes using Merman's default raster settings.
    pub fn render_png(&self, source: impl AsRef<str>) -> Result<Vec<u8>, MermaidError> {
        self.render_png_with(source, &RasterOptions::default())
    }

    /// Render Mermaid source to PNG bytes with custom raster settings.
    pub fn render_png_with(
        &self,
        source: impl AsRef<str>,
        options: &MermaidRasterOptions,
    ) -> Result<Vec<u8>, MermaidError> {
        self.renderer
            .render_png_sync(source.as_ref(), options)
            .map_err(MermaidError::Raster)?
            .ok_or(MermaidError::NoDiagram)
    }

    /// Render Mermaid source to PNG with an explicit resolution and tuicore [`Theme`].
    pub fn render_png_with_theme(
        &self,
        source: impl AsRef<str>,
        options: &MermaidRasterOptions,
        theme: &Theme,
    ) -> Result<Vec<u8>, MermaidError> {
        self.renderer
            .clone()
            .with_host_theme(&mermaid_host_theme(theme))
            .render_png_sync(source.as_ref(), options)
            .map_err(MermaidError::Raster)?
            .ok_or(MermaidError::NoDiagram)
    }

    /// Render Mermaid source into an [`Image`] ready for a tuicore view.
    pub fn render_image(&self, source: impl AsRef<str>) -> Result<Image, MermaidError> {
        self.render_image_with(source, &RasterOptions::default())
    }

    /// Render Mermaid source into an [`Image`] with custom raster settings.
    pub fn render_image_with(
        &self,
        source: impl AsRef<str>,
        options: &MermaidRasterOptions,
    ) -> Result<Image, MermaidError> {
        let png = self.render_png_with(source, options)?;
        Image::from_bytes(png).map_err(MermaidError::Decode)
    }

    /// Render Mermaid source into a themed [`Image`] at the requested raster resolution.
    pub fn render_image_with_theme(
        &self,
        source: impl AsRef<str>,
        options: &MermaidRasterOptions,
        theme: &Theme,
    ) -> Result<Image, MermaidError> {
        let png = self.render_png_with_theme(source, options, theme)?;
        Image::from_bytes(png).map_err(MermaidError::Decode)
    }
}

fn css_color(color: Color) -> Option<String> {
    let (red, green, blue) = color_rgb(color)?;
    Some(format!("#{red:02x}{green:02x}{blue:02x}"))
}

fn is_dark(color: Color) -> bool {
    let Some((red, green, blue)) = color_rgb(color) else {
        return true;
    };
    u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114 < 128_000
}

fn color_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Reset => None,
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((205, 49, 49)),
        Color::Green => Some((13, 188, 121)),
        Color::Yellow => Some((229, 229, 16)),
        Color::Blue => Some((36, 114, 200)),
        Color::Magenta => Some((188, 63, 188)),
        Color::Cyan => Some((17, 168, 205)),
        Color::Gray => Some((229, 229, 229)),
        Color::DarkGray => Some((102, 102, 102)),
        Color::LightRed => Some((241, 76, 76)),
        Color::LightGreen => Some((35, 209, 139)),
        Color::LightYellow => Some((245, 245, 67)),
        Color::LightBlue => Some((59, 142, 234)),
        Color::LightMagenta => Some((214, 112, 214)),
        Color::LightCyan => Some((41, 184, 219)),
        Color::White => Some((255, 255, 255)),
        Color::Indexed(index) => Some(indexed_color_rgb(index)),
        Color::Rgb(red, green, blue) => Some((red, green, blue)),
    }
}

fn indexed_color_rgb(index: u8) -> (u8, u8, u8) {
    if index < 16 {
        const ANSI: [(u8, u8, u8); 16] = [
            (0, 0, 0),
            (205, 49, 49),
            (13, 188, 121),
            (229, 229, 16),
            (36, 114, 200),
            (188, 63, 188),
            (17, 168, 205),
            (229, 229, 229),
            (102, 102, 102),
            (241, 76, 76),
            (35, 209, 139),
            (245, 245, 67),
            (59, 142, 234),
            (214, 112, 214),
            (41, 184, 219),
            (255, 255, 255),
        ];
        return ANSI[usize::from(index)];
    }
    if index >= 232 {
        let gray = 8 + (index - 232) * 10;
        return (gray, gray, gray);
    }
    let index = index - 16;
    let component = |value| if value == 0 { 0 } else { 55 + value * 40 };
    (
        component(index / 36),
        component((index / 6) % 6),
        component(index % 6),
    )
}

/// Errors produced while rendering Mermaid source.
#[derive(Debug)]
pub enum MermaidError {
    /// The input did not contain a Mermaid diagram.
    NoDiagram,
    /// Merman could not parse or render SVG for the diagram.
    Svg(merman::render::HeadlessError),
    /// Merman could not rasterize the diagram.
    Raster(merman::render::raster::RasterError),
    /// The generated PNG could not be decoded as a tuicore [`Image`].
    Decode(ImageError),
}

impl fmt::Display for MermaidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDiagram => f.write_str("source does not contain a Mermaid diagram"),
            Self::Svg(error) => write!(f, "failed to render Mermaid SVG: {error}"),
            Self::Raster(error) => write!(f, "failed to rasterize Mermaid diagram: {error}"),
            Self::Decode(error) => write!(f, "failed to decode Mermaid PNG: {error}"),
        }
    }
}

impl std::error::Error for MermaidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoDiagram => None,
            Self::Svg(error) => Some(error),
            Self::Raster(error) => Some(error),
            Self::Decode(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = "flowchart LR\n    A[Start] --> B[Done]";

    #[test]
    fn renders_mermaid_to_svg_and_png() {
        let renderer = MermaidRenderer::new();

        let svg = renderer
            .render_svg(FLOWCHART)
            .expect("diagram renders to SVG");
        let png = renderer
            .render_png(FLOWCHART)
            .expect("diagram renders to PNG");

        assert!(svg.starts_with("<svg"));
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn renders_a_terminal_image() {
        let image = MermaidRenderer::new()
            .render_image(FLOWCHART)
            .expect("diagram renders to image");

        let (width, height) = image.dimensions();
        assert!(width > 0);
        assert!(height > 0);
    }

    #[test]
    fn renders_a_high_resolution_png_with_semantic_theme_colors() {
        let theme = Theme::default();
        let options = MermaidRasterOptions::default()
            .with_fit_to(MermaidRasterFitBox::contain(960, 540))
            .with_scale(2.0);
        let png = MermaidRenderer::new()
            .render_png_with_theme(FLOWCHART, &options, &theme)
            .expect("themed diagram renders to PNG");
        let profile = mermaid_host_theme(&theme);

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(profile.roles.text.is_some());
        assert!(profile.roles.surface.is_some());
    }

    #[test]
    fn reports_invalid_mermaid_source() {
        let error = MermaidRenderer::new()
            .render_svg("not a diagram")
            .expect_err("plain text is not Mermaid");

        assert!(matches!(error, MermaidError::Svg(_)));
    }
}
