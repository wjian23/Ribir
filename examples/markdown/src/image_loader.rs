//! Remote image loading with async download + decode pipeline
//!
//! Features:
//! - Global cache keyed by URL
//! - Download deduplication via oneshot channels
//! - Multiple waiters receive result when download completes

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use parking_lot::RwLock;
use ribir::prelude::*;
use tokio::sync::oneshot;

use crate::components::InlineNode;

/// Global image cache
static IMAGE_CACHE: once_cell::sync::Lazy<RwLock<ImageCache>> =
  once_cell::sync::Lazy::new(|| RwLock::new(ImageCache::new(100)));

/// Result type for image download
type ImageResult = Result<Image, String>;

/// Waiters for a pending download
type Waiters = Arc<Mutex<Vec<oneshot::Sender<ImageResult>>>>;

/// Cache entry for a remote image
#[derive(Clone)]
enum CacheEntry {
  /// Download in progress - list of waiters
  Pending(Waiters),
  /// Successfully loaded
  Loaded(Image),
  /// Failed to load
  Error(String),
}

/// LRU cache for remote images
struct ImageCache {
  entries: HashMap<String, CacheEntry>,
  /// LRU order (most recent at end)
  order: Vec<String>,
  /// Max entries
  capacity: usize,
}

/// Result of a cache lookup
pub enum CacheResult {
  /// Image already in cache (success or error)
  Ready(ImageResult),
  /// Download in progress, await this future
  Pending(oneshot::Receiver<ImageResult>),
}

impl ImageCache {
  fn new(capacity: usize) -> Self { Self { entries: HashMap::new(), order: Vec::new(), capacity } }

  fn touch(&mut self, url: &str) {
    if let Some(pos) = self.order.iter().position(|u| u == url) {
      self.order.remove(pos);
      self.order.push(url.to_string());
    }
  }

  fn evict(&mut self) {
    while self.entries.len() > self.capacity {
      if let Some(oldest) = self.order.first().cloned() {
        self.entries.remove(&oldest);
        self.order.remove(0);
      }
    }
  }

  /// Get image from cache, or start download if not present
  /// - Returns `CacheResult::Ready(Ok(img))` if cached successfully
  /// - Returns `CacheResult::Ready(Err(msg))` if cached as failed
  /// - Returns `CacheResult::Pending(rx)` if download in progress; caller must
  ///   await the receiver
  fn get_or_start(&mut self, url: &str) -> CacheResult {
    self.touch(url);

    // Check if already cached or pending
    if let Some(entry) = self.entries.get(url) {
      return match entry {
        CacheEntry::Loaded(img) => CacheResult::Ready(Ok(img.clone())),
        CacheEntry::Error(msg) => CacheResult::Ready(Err(msg.clone())),
        CacheEntry::Pending(existing_waiters) => {
          // Download in progress - add to waiters
          let (tx, rx) = oneshot::channel();
          if let Ok(mut senders) = existing_waiters.lock() {
            senders.push(tx);
          }
          CacheResult::Pending(rx)
        }
      };
    }

    // Not cached - create pending entry and spawn download
    let (tx, rx) = oneshot::channel();
    let waiters = Arc::new(Mutex::new(vec![tx]));
    self
      .entries
      .insert(url.to_string(), CacheEntry::Pending(waiters.clone()));
    self.evict();

    // Spawn download task
    let dl_url = url.to_string();
    println!("Initiating download for URL: {}", dl_url);
    AppCtx::spawn_local(async move {
      let result = fetch_and_decode_image(&dl_url).await;
      println!("Download completed for URL: {}", dl_url);

      // Notify all waiters and update cache
      let mut cache = IMAGE_CACHE.write();
      if let Some(CacheEntry::Pending(ws)) = cache.entries.remove(&dl_url) {
        if let Ok(mut senders) = ws.lock() {
          for sender in senders.drain(..) {
            let _ = sender.send(result.clone());
          }
        }
      }

      match result {
        Ok(img) => {
          cache
            .entries
            .insert(dl_url.clone(), CacheEntry::Loaded(img));
        }
        Err(e) => {
          cache.entries.insert(dl_url, CacheEntry::Error(e));
        }
      }
    });

    CacheResult::Pending(rx)
  }
}

/// State of a remote image loading operation
#[derive(Debug, Clone)]
pub enum ImageState {
  Loading,
  Loaded(Image),
  Error(String),
}

/// A widget that loads and displays an image from a remote URL
pub struct RemoteImage {
  pub url: String,
  pub alt: String,
  pub max_width: Option<f32>,
}

impl RemoteImage {
  pub fn new(url: impl Into<String>) -> Self {
    Self { url: url.into(), alt: String::new(), max_width: None }
  }

  pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
    self.alt = alt.into();
    self
  }

  pub fn with_max_width(mut self, width: f32) -> Self {
    self.max_width = Some(width);
    self
  }

  pub fn into_widget(self) -> Widget<'static> {
    let url = self.url.clone();
    let alt = self.alt.clone();
    let max_width = self.max_width;
    let state = request_image_state(&url);

    fn_widget! {
      let alt = alt.clone();
      @ {
        pipe!($read(state);).map(move |_| {
          let alt = alt.clone();
          fn_widget! {
            let w = match $read(state).clone() {
              ImageState::Loading => render_loading_placeholder(&alt),
              ImageState::Loaded(img) => render_loaded_image(img, max_width),
              ImageState::Error(msg) => render_error_placeholder(&msg, &alt),
            };
            w
          }
          .into_widget()
        })
      }
    }
    .into_widget()
  }
}

/// Default max width for markdown images (to prevent oversized images)
pub const DEFAULT_IMAGE_MAX_WIDTH: f32 = 600.0;

/// Render a markdown inline image node to widget
/// This is the main entry point for rendering images from markdown parsing
pub fn render_markdown_image(inline: InlineNode) -> Widget<'static> {
  match inline {
    InlineNode::Image { url, alt } => fn_widget! {
      @ {
        RemoteImage::new(url)
          .with_alt(alt)
          .with_max_width(DEFAULT_IMAGE_MAX_WIDTH)
          .into_widget()
      }
    }
    .into_widget(),
    _ => panic!("Expected Image inline node, got {:?}", inline),
  }
}

async fn fetch_and_decode_image(url: &str) -> ImageResult {
  let response = ::reqwest::get(url)
    .await
    .map_err(|e| format!("Failed to fetch: {}", e))?;

  if !response.status().is_success() {
    return Err(format!("HTTP {}", response.status()));
  }

  let bytes: Vec<u8> = response
    .bytes()
    .await
    .map_err(|e| format!("Failed to read body: {}", e))?
    .to_vec();

  if PixelImage::from_webp(&bytes).is_ok() {
    return Image::new(bytes).map_err(|e| format!("WebP decode failed: {}", e));
  }

  decode_with_image_crate(&bytes)
}

fn decode_with_image_crate(bytes: &[u8]) -> ImageResult {
  use std::io::Cursor;

  use ::image::ImageReader;

  let cursor = Cursor::new(bytes);
  let reader = ImageReader::new(cursor)
    .with_guessed_format()
    .map_err(|e| format!("Failed to detect format: {}", e))?;

  let dynamic_image = reader
    .decode()
    .map_err(|e| format!("Failed to decode: {}", e))?;

  let rgba = dynamic_image.into_rgba8();
  let (width, height) = rgba.dimensions();
  let raw = rgba.into_raw();

  let pixel_img = PixelImage::new(std::borrow::Cow::Owned(raw), width, height, ColorFormat::Rgba8);

  let mut webp_bytes = Vec::new();
  pixel_img
    .write_as_webp(&mut webp_bytes)
    .map_err(|e| format!("WebP encode failed: {}", e))?;
  Image::new(webp_bytes).map_err(|e| format!("Image create failed: {}", e))
}

pub(crate) fn request_image_state(url: &str) -> Stateful<ImageState> {
  let state = Stateful::new(ImageState::Loading);
  let cache_result = {
    let mut cache = IMAGE_CACHE.write();
    cache.get_or_start(url)
  };

  match cache_result {
    CacheResult::Ready(Ok(img)) => {
      *state.write() = ImageState::Loaded(img);
    }
    CacheResult::Ready(Err(msg)) => {
      *state.write() = ImageState::Error(msg);
    }
    CacheResult::Pending(rx) => {
      let state_for_task = state.clone_writer();
      AppCtx::spawn_local(async move {
        match rx.await {
          Ok(result) => match result {
            Ok(img) => *state_for_task.write() = ImageState::Loaded(img),
            Err(e) => *state_for_task.write() = ImageState::Error(e),
          },
          Err(e) => {
            *state_for_task.write() = ImageState::Error(format!("Download task failed: {}", e));
          }
        }
      });
    }
  }

  state
}

fn render_loading_placeholder(alt: &str) -> Widget<'static> {
  let alt = alt.to_string();
  fn_widget! {
    @Column {
      align_items: Align::Center,
      @Container {
        width: 300.0,
        height: 200.0,
        background: Color::from_u32(0xFFF5F5F5),
        @Column {
          align_items: Align::Center,
          @Text { text: "⏳", font_size: 32.0 }
          @Text {
            text: "Loading image...",
            font_size: 12.0,
            foreground: Color::from_u32(0xFF999999),
          }
          @Text {
            text: alt.clone(),
            font_size: 10.0,
            foreground: Color::from_u32(0xFFBBBBBB),
          }
        }
      }
    }
  }
  .into_widget()
}

fn render_loaded_image(img: Image, max_width: Option<f32>) -> Widget<'static> {
  // Calculate scaled size to preserve aspect ratio
  let (display_width, display_height) = if let Some(max_w) = max_width {
    let img_w = img.width() as f32;
    let img_h = img.height() as f32;
    if img_w > max_w {
      let scale = max_w / img_w;
      (max_w, img_h * scale)
    } else {
      (img_w, img_h)
    }
  } else {
    (img.width() as f32, img.height() as f32)
  };

  fn_widget! {
    @Container {
      size: Size::new(display_width, display_height),
      @FatObj {
        box_fit: BoxFit::Contain,
        @ { img }
      }
    }
  }
  .into_widget()
}

fn render_error_placeholder(msg: &str, alt: &str) -> Widget<'static> {
  let msg = msg.to_string();
  let alt = alt.to_string();
  fn_widget! {
    @Column {
      align_items: Align::Center,
      @Container {
        width: 300.0,
        height: 150.0,
        background: Color::from_u32(0xFFFFF0F0),
        @Column {
          align_items: Align::Center,
          @Text { text: "❌", font_size: 24.0 }
          @Text {
            text: "Failed to load image",
            font_size: 12.0,
            foreground: Color::from_u32(0xFFFF6666),
          }
          @Text {
            text: msg.clone(),
            font_size: 10.0,
            foreground: Color::from_u32(0xFF999999),
          }
          @Text {
            text: alt.clone(),
            font_size: 10.0,
            foreground: Color::from_u32(0xFFBBBBBB),
          }
        }
      }
    }
  }
  .into_widget()
}
