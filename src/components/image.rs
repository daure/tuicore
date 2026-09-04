use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::sync::{Arc, OnceLock};

use base64::Engine;
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image as RatatuiImage, Resize};

use crate::runtime::renderer::{
    BASE_DIRECT_KITTY_Z_INDEX, DirectKittyIntent, DirectKittyPlacementId, GraphicsLevel,
    next_direct_kitty_image_id,
};
use crate::{
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, RenderCtx, TickResult, TuiNode,
};

/// Selects the terminal graphics protocol used to render an [`Image`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Detect a compatible protocol offered by the terminal.
    #[default]
    Auto,
    /// Use the Sixel graphics protocol.
    Sixel,
    /// Use renderer-owned direct Kitty placements.
    Kitty,
    /// Use ratatui-image's cell-bound Kitty Unicode placeholders.
    ///
    /// Unlike [`Self::Kitty`], placeholders participate in normal buffer rendering, so dialogs
    /// and other overlays occlude them naturally.
    KittyPlaceholder,
}

/// A decoded terminal image that can be loaded from a file, URL, or base64 data.
///
/// Image decoding and URL loading happen at construction time. Protocol encoding happens during
/// layout, keeping rendering read-only.
pub struct Image {
    image: DynamicImage,
    protocol: Option<Protocol>,
    direct_kitty: Option<DirectKittyImage>,
    pending_kitty: Option<PendingKittyImage>,
    picker: Option<Picker>,
    graphics_protocol: ImageProtocol,
    size: Size,
    encoded_size: Option<Size>,
}

struct PendingKittyImage {
    size: Size,
    receiver: Receiver<Result<DirectKittyImage, image::ImageError>>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.image.width())
            .field("height", &self.image.height())
            .field("graphics_protocol", &self.graphics_protocol)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ImageError {
    Read(std::io::Error),
    Download(reqwest::Error),
    Decode(image::ImageError),
    Base64(base64::DecodeError),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "failed to read image: {error}"),
            Self::Download(error) => write!(f, "failed to download image: {error}"),
            Self::Decode(error) => write!(f, "failed to decode image: {error}"),
            Self::Base64(error) => write!(f, "failed to decode base64 image: {error}"),
        }
    }
}

impl std::error::Error for ImageError {}

impl Image {
    /// Load an image from a local path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ImageError> {
        let bytes = std::fs::read(path).map_err(ImageError::Read)?;
        Self::from_bytes(bytes)
    }

    /// Download and load an image from an HTTP(S) URL.
    pub fn from_url(url: impl AsRef<str>) -> Result<Self, ImageError> {
        let bytes = reqwest::blocking::get(url.as_ref())
            .map_err(ImageError::Download)?
            .error_for_status()
            .map_err(ImageError::Download)?
            .bytes()
            .map_err(ImageError::Download)?;
        Self::from_bytes(bytes)
    }

    /// Download and load an image from an HTTP(S) URL using HTTP Basic authentication.
    pub fn from_url_with_basic_auth(
        url: impl AsRef<str>,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<Self, ImageError> {
        let bytes = reqwest::blocking::Client::new()
            .get(url.as_ref())
            .basic_auth(username.as_ref(), Some(password.as_ref()))
            .send()
            .map_err(ImageError::Download)?
            .error_for_status()
            .map_err(ImageError::Download)?
            .bytes()
            .map_err(ImageError::Download)?;
        Self::from_bytes(bytes)
    }

    /// Decode a base64 image payload. `data:image/...;base64,...` URLs are also accepted.
    pub fn from_base64(encoded: impl AsRef<str>) -> Result<Self, ImageError> {
        let encoded = encoded.as_ref();
        let payload = encoded
            .rsplit_once(',')
            .map_or(encoded, |(_, payload)| payload);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(ImageError::Base64)?;
        Self::from_bytes(bytes)
    }

    /// Decode image bytes already available in memory.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, ImageError> {
        let image = image::load_from_memory(bytes.as_ref()).map_err(ImageError::Decode)?;
        Ok(Self {
            image,
            protocol: None,
            direct_kitty: None,
            pending_kitty: None,
            picker: None,
            graphics_protocol: ImageProtocol::Auto,
            size: Size::new(32, 16),
            encoded_size: None,
        })
    }

    /// Prefer a graphics protocol instead of terminal detection.
    pub fn protocol(mut self, protocol: ImageProtocol) -> Self {
        self.graphics_protocol = protocol;
        self.protocol = None;
        self.direct_kitty = None;
        self.pending_kitty = None;
        self.picker = None;
        self.encoded_size = None;
        self
    }

    /// Return the configured graphics protocol.
    pub const fn graphics_protocol(&self) -> ImageProtocol {
        self.graphics_protocol
    }

    /// Set the preferred size in terminal cells when the parent uses fit-content sizing.
    pub const fn size(mut self, width: u16, height: u16) -> Self {
        self.size = Size::new(width, height);
        self
    }

    /// Begin encoding a Kitty image for the given terminal-cell size without blocking rendering.
    pub fn preload(&mut self, width: u16, height: u16) {
        if self.graphics_protocol == ImageProtocol::Kitty {
            self.request_kitty_image(Size::new(width, height));
        }
    }

    /// Return the decoded image dimensions in pixels.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.image.width(), self.image.height())
    }

    /// Request that the current graphics payload is emitted on the next render.
    pub fn redraw(&mut self) {
        if let Some(Protocol::Sixel(sixel)) = &mut self.protocol {
            sixel.data.push_str("\x1b[0m");
        }
        if let Some(kitty) = &mut self.direct_kitty {
            kitty.redraw();
        }
    }

    /// Return a terminal command that removes the current Kitty image placement.
    pub fn kitty_cleanup_command(&self) -> Option<String> {
        self.direct_kitty
            .as_ref()
            .map(DirectKittyImage::cleanup_command)
    }

    /// Poll a background Kitty image encode.
    pub fn tick(&mut self) -> TickResult {
        let Some(pending) = &self.pending_kitty else {
            return TickResult::IDLE;
        };
        match pending.receiver.try_recv() {
            Ok(Ok(image)) => {
                self.direct_kitty = Some(image);
                self.pending_kitty = None;
                TickResult::CHANGED
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.pending_kitty = None;
                TickResult::CHANGED
            }
            Err(TryRecvError::Empty) => TickResult::ACTIVE,
        }
    }

    fn picker(&mut self) -> &Picker {
        let picker = self.picker.get_or_insert_with(detected_picker);
        match self.graphics_protocol {
            ImageProtocol::Auto => {}
            ImageProtocol::Sixel => picker.set_protocol_type(ProtocolType::Sixel),
            ImageProtocol::Kitty | ImageProtocol::KittyPlaceholder => {
                picker.set_protocol_type(ProtocolType::Kitty);
            }
        }
        picker
    }

    fn encode(&mut self, area: Rect) {
        if area.is_empty() {
            self.protocol = None;
            self.direct_kitty = None;
            self.pending_kitty = None;
            self.encoded_size = None;
            return;
        }

        let size = area.into();
        if self.encoded_size == Some(size) {
            return;
        }

        if self.graphics_protocol == ImageProtocol::Kitty {
            self.request_kitty_image(size);
            self.protocol = None;
        } else {
            let picker = self.picker().clone();
            self.protocol = picker
                .new_protocol(self.image.clone(), size, Resize::Scale(None))
                .ok();
            self.direct_kitty = None;
        }
        self.encoded_size = Some(size);
    }

    fn request_kitty_image(&mut self, size: Size) {
        if self.encoded_size == Some(size)
            || self
                .pending_kitty
                .as_ref()
                .is_some_and(|pending| pending.size == size)
        {
            return;
        }
        let image = self.image.clone();
        let (sender, receiver) = sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(DirectKittyImage::new(image, size));
        });
        self.pending_kitty = Some(PendingKittyImage { size, receiver });
        self.encoded_size = Some(size);
    }
}

struct DirectKittyImage {
    data: Arc<str>,
    size: Size,
    requested_size: Size,
    image_id: u32,
    placement_id: u32,
    generation: u64,
}

impl DirectKittyImage {
    fn new(image: DynamicImage, size: Size) -> Result<Self, image::ImageError> {
        let requested_size = size;
        let (width, height, size) = fitted_kitty_size(&image, size);
        let image = image.resize_exact(width, height, FilterType::Triangle);
        let mut png = Vec::new();
        image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;

        let image_id = next_direct_kitty_image_id();
        let placement_id = 1;
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        let mut data = String::new();
        for (index, chunk) in encoded.as_bytes().chunks(4096).enumerate() {
            let more = usize::from((index + 1) * 4096 < encoded.len());
            if index == 0 {
                data.push_str(&format!("\x1b_Ga=t,t=d,f=100,i={image_id},q=2,m={more};"));
            } else {
                data.push_str(&format!("\x1b_Gm={more};"));
            }
            data.push_str(std::str::from_utf8(chunk).expect("base64 output is UTF-8"));
            data.push_str("\x1b\\");
        }

        Ok(Self {
            data: Arc::from(data),
            size,
            requested_size,
            image_id,
            placement_id,
            generation: 0,
        })
    }

    fn redraw(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    fn cleanup_command(&self) -> String {
        format!(
            "\x1b_Ga=d,d=I,i={},p={},q=2\x1b\\",
            self.image_id, self.placement_id
        )
    }

    fn intent(&self, area: Rect) -> Option<DirectKittyIntent> {
        if Size::from(area) != self.requested_size {
            return None;
        }
        Some(DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id: self.image_id,
                placement_id: self.placement_id,
            },
            area: Rect::new(area.x, area.y, self.size.width, self.size.height),
            generation: self.generation,
            payload: Arc::clone(&self.data),
            level: GraphicsLevel::base(),
            z_index: BASE_DIRECT_KITTY_Z_INDEX,
        })
    }
}

fn fitted_kitty_size(image: &DynamicImage, area: Size) -> (u32, u32, Size) {
    let max_width = u32::from(area.width).saturating_mul(10);
    let max_height = u32::from(area.height).saturating_mul(20);
    let scale = f64::min(
        max_width as f64 / image.width() as f64,
        max_height as f64 / image.height() as f64,
    );
    let width = (image.width() as f64 * scale).round().max(1.0) as u32;
    let height = (image.height() as f64 * scale).round().max(1.0) as u32;
    let size = Size::new(
        width.div_ceil(10).min(u32::from(area.width)) as u16,
        height.div_ceil(20).min(u32::from(area.height)) as u16,
    );

    (width, height, size)
}

fn detected_picker() -> Picker {
    static PICKER: OnceLock<Picker> = OnceLock::new();

    PICKER
        .get_or_init(|| Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()))
        .clone()
}

impl<M> TuiNode<M> for Image {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(self.size.width, self.size.height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        self.encode(area);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        if let Some(kitty) = &self.direct_kitty {
            if let Some(intent) = kitty.intent(area) {
                ctx.register_direct_kitty(intent);
            }
        } else if let Some(protocol) = &self.protocol {
            frame.render_widget(RatatuiImage::new(protocol).allow_clipping(true), area);
        }
    }

    fn init(&mut self, _ctx: &mut crate::LifecycleCtx<M>) {
        self.picker();
    }

    fn tick(
        &mut self,
        _dt: std::time::Duration,
        _settings: crate::AnimationSettings,
    ) -> TickResult {
        Image::tick(self)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    const TEST_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAGAAAAAwCAIAAABhdOiYAAAAf0lEQVR42u3RMRGAQBADwBdBTU1NjTDkvAhcffMSMJFrbjYTA9mM47wjvd4v0j2fSFcoAxAgQIAAAQIECBAgQIAKgLoOSx0PCBAgQIAAAQIECBAgQBVAXYeljgcECBAgQIAAAQIECBCgCqCuw1LHAwIECBAgQIAAAQIECFAB0A/Lrzglvf/PRwAAAABJRU5ErkJggg==";

    #[test]
    fn decodes_raw_base64_image_data() {
        let image = Image::from_base64(TEST_PNG).expect("valid PNG data");

        assert_eq!(image.dimensions(), (96, 48));
    }

    #[test]
    fn decodes_data_url_base64_image_data() {
        let image = Image::from_base64(format!("data:image/png;base64,{TEST_PNG}"))
            .expect("valid PNG data URL");

        assert_eq!(image.dimensions(), (96, 48));
    }

    #[test]
    fn loads_a_url_with_basic_authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image.png", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: basic dxnlckblegftcgxllmnvbtp0b2tlbg=="),
                "{request}"
            );
            let body = base64::engine::general_purpose::STANDARD
                .decode(TEST_PNG)
                .unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        let image = Image::from_url_with_basic_auth(url, "user@example.com", "token")
            .expect("authenticated image download succeeds");

        server.join().unwrap();
        assert_eq!(image.dimensions(), (96, 48));
    }

    #[test]
    fn does_not_reencode_when_only_the_layout_position_changes() {
        let mut image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        image.picker = Some(Picker::halfblocks());
        let area = Rect::new(0, 0, 32, 16);

        image.encode(area);
        image.protocol = None;
        image.encode(Rect::new(10, 5, 32, 16));

        assert!(image.protocol.is_none());
    }

    #[test]
    fn redraw_changes_the_cached_sixel_payload() {
        let mut image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        image.protocol = Some(Protocol::Sixel(ratatui_image::protocol::sixel::Sixel {
            data: "payload".to_string(),
            size: Size::new(1, 1),
            is_tmux: false,
        }));

        image.redraw();

        let Some(Protocol::Sixel(sixel)) = &image.protocol else {
            panic!("Sixel protocol is preserved");
        };
        assert_eq!(sixel.data, "payload\x1b[0m");
    }

    #[test]
    fn direct_kitty_payload_transmits_without_creating_a_placement() {
        let image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        let kitty =
            DirectKittyImage::new(image.image, Size::new(2, 1)).expect("PNG encoding succeeds");

        assert!(kitty.data.starts_with("\x1b_Ga=t,t=d,f=100,"));
        assert!(!kitty.data.contains("c="));
        assert!(!kitty.data.contains("r="));
        assert!(!kitty.data.contains("z="));
        assert!(!kitty.data.contains('\u{10eeee}'));
    }

    #[test]
    fn direct_kitty_fits_the_image_ratio_inside_the_available_cells() {
        let image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        let (_, _, size) = fitted_kitty_size(&image.image, Size::new(4, 4));

        assert_eq!(size, Size::new(4, 1));
    }

    #[test]
    fn direct_kitty_can_remove_an_existing_placement() {
        let image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        let kitty =
            DirectKittyImage::new(image.image, Size::new(2, 1)).expect("PNG encoding succeeds");

        assert!(kitty.cleanup_command().starts_with("\x1b_Ga=d,d=I,i="));
        assert!(kitty.cleanup_command().ends_with(",p=1,q=2\x1b\\"));
    }

    #[test]
    fn direct_kitty_redraw_advances_the_cached_placement_generation() {
        let image = Image::from_base64(TEST_PNG).expect("valid PNG data");
        let mut kitty =
            DirectKittyImage::new(image.image, Size::new(2, 1)).expect("PNG encoding succeeds");

        kitty.redraw();

        assert_eq!(kitty.generation, 1);
        assert!(kitty.data.starts_with("\x1b_Ga=t,t=d,f=100,"));
    }

    #[test]
    fn kitty_placeholders_use_ratatui_image_instead_of_direct_kitty() {
        let mut image = Image::from_base64(TEST_PNG)
            .expect("valid PNG data")
            .protocol(ImageProtocol::KittyPlaceholder);
        image.picker = Some(Picker::halfblocks());

        image.encode(Rect::new(0, 0, 32, 16));

        assert!(matches!(image.protocol, Some(Protocol::Kitty(_))));
        assert!(image.direct_kitty.is_none());
        assert!(image.pending_kitty.is_none());
    }
}
