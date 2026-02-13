//! Background font rasterization worker.
//!
//! [`FontWorker`] offloads glyph rasterization to a dedicated background
//! thread, producing antialiased alpha bitmaps that are inserted into the
//! renderer's [`GlyphAtlas`].
//!
//! The built-in 8×16 bitmap font is used as the source glyph data.  Glyphs
//! are upscaled to the requested pixel size via 4× supersampled rendering
//! with box-filter downsampling, producing smooth antialiased edges at any
//! size — from tiny 10px UI labels to large 32px title text.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────┐   GlyphRequest    ┌────────────────┐
//! │  Renderer  │ ───────────────►  │  FontWorker     │
//! │ (main/     │                   │  (background    │
//! │  render    │ ◄───────────────  │   thread)       │
//! │  thread)   │   RasterizedGlyph │                 │
//! └────────────┘                   └────────────────┘
//! ```
//!
//! The renderer calls [`FontWorker::request_glyph`] to queue glyphs for
//! rasterization and [`FontWorker::poll_results`] to drain completed
//! results into the glyph atlas.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::bitmap_font::BitmapFont;
use crate::glyph::{GlyphKey, GlyphMetrics};

// ---------------------------------------------------------------------------
// Types exchanged between renderer and worker
// ---------------------------------------------------------------------------

/// A request to rasterize a single glyph at a specific size.
struct GlyphRequest {
    key: GlyphKey,
    /// Character to rasterize.
    codepoint: char,
    /// Target glyph height in pixels.
    target_height: u32,
}

/// A completed rasterized glyph returned from the worker.
pub(crate) struct RasterizedGlyph {
    pub key: GlyphKey,
    /// Alpha-only bitmap data (width × height bytes).
    pub bitmap: Vec<u8>,
    pub metrics: GlyphMetrics,
}

/// Messages sent to the worker thread.
enum WorkerMsg {
    Rasterize(GlyphRequest),
    Shutdown,
}

// ---------------------------------------------------------------------------
// FontWorker
// ---------------------------------------------------------------------------

/// Manages a background thread that rasterizes glyphs asynchronously.
///
/// Glyphs are rendered by upscaling the 8×16 bitmap font source data at 4×
/// resolution and box-filter downsampling to produce antialiased alpha maps.
pub(crate) struct FontWorker {
    /// Channel to send requests to the worker thread.
    request_tx: mpsc::Sender<WorkerMsg>,
    /// Channel to receive completed glyphs from the worker thread.
    result_rx: mpsc::Receiver<RasterizedGlyph>,
    /// Worker thread handle — joined on drop.
    handle: Option<JoinHandle<()>>,
    /// Set of glyph keys currently being processed (avoid duplicate requests).
    pending: HashSet<GlyphKey>,
}

impl FontWorker {
    /// Spawn the background font rasterization worker thread.
    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<WorkerMsg>();
        let (res_tx, res_rx) = mpsc::channel::<RasterizedGlyph>();

        let handle = thread::Builder::new()
            .name("font-worker".into())
            .spawn(move || Self::worker_loop(req_rx, res_tx))
            .expect("failed to spawn font worker thread");

        Self {
            request_tx: req_tx,
            result_rx: res_rx,
            handle: Some(handle),
            pending: HashSet::new(),
        }
    }

    /// Submit a glyph rasterization request.
    ///
    /// If the glyph is already pending, the request is silently skipped.
    pub fn request_glyph(&mut self, key: GlyphKey, codepoint: char, target_height: u32) {
        if self.pending.contains(&key) {
            return;
        }
        let req = GlyphRequest {
            key,
            codepoint,
            target_height,
        };
        if self.request_tx.send(WorkerMsg::Rasterize(req)).is_ok() {
            self.pending.insert(key);
        }
    }

    /// Drain all completed glyph results from the worker.
    ///
    /// Returns a vector of rasterized glyphs ready for atlas insertion.
    pub fn poll_results(&mut self) -> Vec<RasterizedGlyph> {
        let mut results = Vec::new();
        while let Ok(glyph) = self.result_rx.try_recv() {
            self.pending.remove(&glyph.key);
            results.push(glyph);
        }
        results
    }

    /// Whether a specific glyph is currently pending.
    pub fn is_pending(&self, key: &GlyphKey) -> bool {
        self.pending.contains(key)
    }

    /// The worker thread's main loop.
    fn worker_loop(
        rx: mpsc::Receiver<WorkerMsg>,
        tx: mpsc::Sender<RasterizedGlyph>,
    ) {
        let font = BitmapFont::new();

        loop {
            let first = match rx.recv() {
                Ok(msg) => msg,
                Err(_) => break,
            };

            match first {
                WorkerMsg::Shutdown => break,
                WorkerMsg::Rasterize(req) => {
                    // Drain additional pending messages, keeping latest per key.
                    let mut batch: HashMap<GlyphKey, GlyphRequest> = HashMap::new();
                    batch.insert(req.key, req);

                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            WorkerMsg::Shutdown => return,
                            WorkerMsg::Rasterize(r) => {
                                batch.insert(r.key, r);
                            }
                        }
                    }

                    // Process all unique glyph requests.
                    for (_, request) in batch {
                        let result = Self::rasterize_glyph(&font, &request);
                        if tx.send(result).is_err() {
                            return; // receiver dropped
                        }
                    }
                }
            }
        }
    }

    /// Rasterize a single glyph using 4× supersampled box-filter downsampling.
    ///
    /// Takes the 8×16 source glyph, renders it at 4× the target size, then
    /// box-filter downsamples to produce smooth antialiased alpha values.
    fn rasterize_glyph(font: &BitmapFont, req: &GlyphRequest) -> RasterizedGlyph {
        let src_w = BitmapFont::GLYPH_WIDTH; // 8
        let src_h = BitmapFont::GLYPH_HEIGHT; // 16

        // Target dimensions maintaining 8:16 = 1:2 aspect ratio.
        let target_h = req.target_height.max(4);
        let target_w = (target_h * src_w + src_h - 1) / src_h; // ceil(h * 8/16) = ceil(h/2)

        // Supersample at 4× resolution for antialiasing.
        let ss_factor = 4_u32;
        let ss_w = target_w * ss_factor;
        let ss_h = target_h * ss_factor;

        let glyph_data = font.glyph(req.codepoint);

        // Render glyph at supersample resolution using nearest-neighbour
        // from the source 8×16 bitmap.
        let mut ss_buf = vec![0u8; (ss_w * ss_h) as usize];

        for sy in 0..ss_h {
            // Map supersample row to source row.
            let src_row = (sy * src_h) / ss_h;
            let bits = glyph_data[src_row.min(src_h - 1) as usize];
            if bits == 0 {
                continue;
            }
            for sx in 0..ss_w {
                let src_col = (sx * src_w) / ss_w;
                if bits & (0x80 >> src_col.min(7)) != 0 {
                    ss_buf[(sy * ss_w + sx) as usize] = 255;
                }
            }
        }

        // Box-filter downsample: average each ss_factor × ss_factor block.
        let mut alpha_buf = vec![0u8; (target_w * target_h) as usize];
        let ss_area = (ss_factor * ss_factor) as u32;

        for ty in 0..target_h {
            for tx in 0..target_w {
                let mut sum = 0u32;
                for dy in 0..ss_factor {
                    for dx in 0..ss_factor {
                        let sx = tx * ss_factor + dx;
                        let sy = ty * ss_factor + dy;
                        sum += ss_buf[(sy * ss_w + sx) as usize] as u32;
                    }
                }
                alpha_buf[(ty * target_w + tx) as usize] = (sum / ss_area) as u8;
            }
        }

        // Compute advance: proportional to target width.
        let advance = target_w as f32;

        RasterizedGlyph {
            key: req.key,
            bitmap: alpha_buf,
            metrics: GlyphMetrics {
                width: target_w,
                height: target_h,
                bearing_x: 0,
                bearing_y: target_h as i32,
                advance,
            },
        }
    }
}

impl Drop for FontWorker {
    fn drop(&mut self) {
        let _ = self.request_tx.send(WorkerMsg::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
