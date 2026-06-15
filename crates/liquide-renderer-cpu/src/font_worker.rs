//! Background font rasterization worker.
//!
//! [`FontWorker`] offloads glyph rasterization to a dedicated background
//! thread, producing antialiased alpha bitmaps that are inserted into the
//! renderer's [`GlyphAtlas`].
//!
//! When a [`FontDatabase`] is provided, glyphs are rasterized using real
//! TrueType/OpenType outlines via `ab_glyph`.  When no matching font face
//! can be resolved, the worker falls back to the built-in 8×16 bitmap font
//! with 4× supersampled box-filter downsampling.
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
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use liquide_font_rasterizer::database::{FontDatabase, FontFaceId};
use liquide_font_rasterizer::rasterize::{GlyphRasterizer, RasterConfig};

use crate::bitmap_font::BitmapFont;
use crate::glyph::{GlyphKey, GlyphMetrics};

/// Shear angle (degrees) used when synthesizing oblique text from an upright
/// face because no true italic face is available. Matches the typographic
/// convention used by `SynthesisConfig::italic` (12°).
const SYNTHETIC_OBLIQUE_DEGREES: f32 = 12.0;

// ---------------------------------------------------------------------------
// Types exchanged between renderer and worker
// ---------------------------------------------------------------------------

/// A request to rasterize a single glyph at a specific size.
struct GlyphRequest {
    key: GlyphKey,
    /// Worker generation when this request was enqueued.
    generation: u64,
    /// Character to rasterize.
    codepoint: char,
    /// Target glyph height in pixels.
    target_height: u32,
    /// Font family name (empty = bitmap fallback).
    font_family: String,
    /// Font weight (100–900).
    font_weight: u16,
    /// Whether italic/oblique styling is requested for this glyph.
    italic: bool,
}

/// A completed rasterized glyph returned from the worker.
pub(crate) struct RasterizedGlyph {
    pub key: GlyphKey,
    pub generation: u64,
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
/// Uses TrueType/OpenType fonts via `GlyphRasterizer` when available,
/// falling back to the built-in 8×16 bitmap font with 4× supersampled
/// box-filter downsampling.
#[allow(dead_code)]
pub(crate) struct FontWorker {
    /// Channel to send requests to the worker thread.
    request_tx: mpsc::Sender<WorkerMsg>,
    /// Channel to receive completed glyphs from the worker thread.
    result_rx: mpsc::Receiver<RasterizedGlyph>,
    /// Worker thread handle — joined on drop.
    handle: Option<JoinHandle<()>>,
    /// Set of glyph keys currently being processed (avoid duplicate requests).
    pending: HashSet<GlyphKey>,
    /// Monotonic cache generation used to ignore stale in-flight glyph results.
    generation: u64,
    /// Shared font database — also held by the worker thread.
    font_db: Arc<Mutex<FontDatabase>>,
}

impl FontWorker {
    /// Spawn the background font rasterization worker thread.
    pub fn new() -> Self {
        Self::with_font_db(FontDatabase::new())
    }

    /// Spawn the worker with a pre-loaded font database.
    pub fn with_font_db(db: FontDatabase) -> Self {
        let font_db = Arc::new(Mutex::new(db));
        let db_clone = Arc::clone(&font_db);

        let (req_tx, req_rx) = mpsc::channel::<WorkerMsg>();
        let (res_tx, res_rx) = mpsc::channel::<RasterizedGlyph>();

        let handle = match thread::Builder::new()
            .name("font-worker".into())
            .spawn(move || Self::worker_loop(req_rx, res_tx, db_clone))
        {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::error!(
                    "failed to spawn font worker thread: {e}; glyph rasterization disabled"
                );
                None
            }
        };

        Self {
            request_tx: req_tx,
            result_rx: res_rx,
            handle,
            pending: HashSet::new(),
            generation: 0,
            font_db,
        }
    }

    /// Submit a glyph rasterization request.
    ///
    /// If the glyph is already pending, the request is silently skipped.
    #[allow(dead_code)]
    pub fn request_glyph(&mut self, key: GlyphKey, codepoint: char, target_height: u32) {
        self.request_glyph_with_font(key, codepoint, target_height, String::new(), 400, false);
    }

    /// Submit a glyph rasterization request with font family/weight/style info.
    ///
    /// When `italic` is set, the worker selects a real italic face if one is
    /// available for the family/weight; otherwise it synthesizes an oblique
    /// slant (shear transform) from the upright face.
    #[allow(clippy::too_many_arguments)]
    pub fn request_glyph_with_font(
        &mut self,
        key: GlyphKey,
        codepoint: char,
        target_height: u32,
        font_family: String,
        font_weight: u16,
        italic: bool,
    ) {
        if self.pending.contains(&key) {
            return;
        }
        let req = GlyphRequest {
            key,
            generation: self.generation,
            codepoint,
            target_height,
            font_family,
            font_weight,
            italic,
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
            if glyph.generation == self.generation {
                self.pending.remove(&glyph.key);
                results.push(glyph);
            }
        }
        results
    }

    /// Return file-backed font faces whose sources changed since load.
    pub fn stale_faces(&self) -> Vec<FontFaceId> {
        let db = self
            .font_db
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        db.stale_faces()
    }

    /// Reload the given file-backed faces from disk and invalidate worker-side
    /// glyph state so subsequent rasterization reads the **fresh** bytes.
    ///
    /// Reloading happens on the shared [`FontDatabase`] (the worker thread holds
    /// the same `Arc`), so the next `Rasterize` batch re-parses the updated
    /// outlines. Bumping the generation discards any in-flight glyphs that were
    /// rasterized from the previous bytes, and clearing `pending` lets the
    /// renderer re-request them. Returns the IDs whose bytes were actually
    /// replaced (a vanished/corrupt source is skipped, leaving last-good bytes).
    pub fn reload_faces<I>(&mut self, face_ids: I) -> Vec<FontFaceId>
    where
        I: IntoIterator<Item = FontFaceId>,
    {
        let face_ids: Vec<FontFaceId> = face_ids.into_iter().collect();
        if face_ids.is_empty() {
            return Vec::new();
        }

        let reloaded = {
            let mut db = self
                .font_db
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let mut reloaded = Vec::with_capacity(face_ids.len());
            for &face_id in &face_ids {
                match db.reload_face(face_id) {
                    Ok(true) => reloaded.push(face_id),
                    Ok(false) => {}
                    Err(err) => {
                        tracing::warn!(
                            face_id = face_id.0,
                            error = %err,
                            "font face reload failed; keeping last-good bytes"
                        );
                    }
                }
            }
            reloaded
        };

        // Even if a reload failed, still invalidate worker-side state for the
        // requested faces so stale in-flight glyphs are not committed.
        self.invalidate_faces(face_ids);
        reloaded
    }

    /// Invalidate worker-side state associated with a set of font faces.
    ///
    /// Drops any in-flight glyph results (by bumping the generation) and clears
    /// the pending set so the renderer re-requests glyphs. NOTE: this does not
    /// itself re-read the font bytes — use [`reload_faces`](Self::reload_faces)
    /// when the on-disk source changed, otherwise re-rasterization reuses the
    /// same cached bytes (the t49-e3-F15 trap).
    pub fn invalidate_faces<I>(&mut self, face_ids: I)
    where
        I: IntoIterator<Item = FontFaceId>,
    {
        let face_ids: Vec<FontFaceId> = face_ids.into_iter().collect();
        if face_ids.is_empty() {
            return;
        }

        self.pending.clear();
        self.generation = self.generation.wrapping_add(1);
        while self.result_rx.try_recv().is_ok() {}
    }

    /// Check for stale file-backed faces, **reload** their bytes from disk, and
    /// invalidate worker-side glyph state.
    ///
    /// Returns the faces that were detected stale (whether or not every reload
    /// succeeded). This closes t49-e3-F15: previously the worker re-rasterized
    /// the same stale bytes forever because nothing re-read the file.
    pub fn invalidate_stale_faces(&mut self) -> Vec<FontFaceId> {
        let stale_faces = self.stale_faces();
        if !stale_faces.is_empty() {
            self.reload_faces(stale_faces.iter().copied());
        }
        stale_faces
    }

    /// Whether a specific glyph is currently pending.
    #[allow(dead_code)]
    pub fn is_pending(&self, key: &GlyphKey) -> bool {
        self.pending.contains(key)
    }

    /// Get a reference to the shared font database.
    #[allow(dead_code)]
    pub fn font_db(&self) -> &Arc<Mutex<FontDatabase>> {
        &self.font_db
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// The worker thread's main loop.
    fn worker_loop(
        rx: mpsc::Receiver<WorkerMsg>,
        tx: mpsc::Sender<RasterizedGlyph>,
        font_db: Arc<Mutex<FontDatabase>>,
    ) {
        let bitmap_font = BitmapFont::new();
        let raster_config = RasterConfig::default();

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

                    // Process all unique glyph requests — try real fonts first,
                    // fall back to bitmap font.
                    let db = font_db.lock().unwrap_or_else(|poison| poison.into_inner());
                    let rasterizer = GlyphRasterizer::new(&db);

                    for (_, request) in batch {
                        let result = Self::rasterize_glyph_real(
                            &rasterizer,
                            &raster_config,
                            &db,
                            &bitmap_font,
                            &request,
                        );
                        if tx.send(result).is_err() {
                            return; // receiver dropped
                        }
                    }
                }
            }
        }
    }

    /// Rasterize a glyph using TrueType/OpenType outlines when available,
    /// falling back to the bitmap font.
    fn rasterize_glyph_real(
        rasterizer: &GlyphRasterizer<'_>,
        config: &RasterConfig,
        db: &FontDatabase,
        bitmap_font: &BitmapFont,
        req: &GlyphRequest,
    ) -> RasterizedGlyph {
        // Try to resolve a real font face if a family was specified.
        if !req.font_family.is_empty() {
            // Ask the database for an italic face when requested. The resolver
            // prefers an exact italic match, then falls back to the upright
            // face for the same family/weight (database::resolve_exact).
            if let Some(face_id) = db.resolve(&req.font_family, req.font_weight, req.italic) {
                let size_px = req.target_height as f32;
                if let Ok(mut glyph_bitmap) =
                    rasterizer.rasterize(face_id, req.codepoint, size_px, config)
                {
                    // If italic was requested but the resolved face is NOT a
                    // true italic face, synthesize the slant via a shear
                    // transform so the run still renders distinctly italic.
                    let face_is_italic = db.get(face_id).map(|f| f.italic).unwrap_or(false);
                    if req.italic && !face_is_italic {
                        glyph_bitmap = liquide_font_rasterizer::synthesis::apply_synthetic_oblique(
                            &glyph_bitmap,
                            SYNTHETIC_OBLIQUE_DEGREES,
                        );
                    }
                    return RasterizedGlyph {
                        key: req.key,
                        generation: req.generation,
                        bitmap: glyph_bitmap.pixels.to_vec(),
                        metrics: GlyphMetrics {
                            width: glyph_bitmap.width,
                            height: glyph_bitmap.height,
                            bearing_x: glyph_bitmap.bearing_x.round() as i32,
                            bearing_y: glyph_bitmap.bearing_y.round() as i32,
                            advance: glyph_bitmap.advance,
                        },
                    };
                }
            }
        }

        // Fallback: supersampled bitmap font.
        Self::rasterize_glyph_bitmap(bitmap_font, req)
    }

    /// Rasterize a single glyph using 4× supersampled box-filter downsampling.
    ///
    /// Takes the 8×16 source glyph, renders it at 4× the target size, then
    /// box-filter downsamples to produce smooth antialiased alpha values.
    fn rasterize_glyph_bitmap(font: &BitmapFont, req: &GlyphRequest) -> RasterizedGlyph {
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
            generation: req.generation,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_font_bytes() -> Option<Vec<u8>> {
        let candidates = [
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\calibri.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ];
        candidates.iter().find_map(|path| {
            let data = std::fs::read(path).ok()?;
            // Validate it parses as a font via the database's own loader so we
            // don't pull ab_glyph in as a direct dev-dependency here.
            let mut probe = FontDatabase::new();
            probe.load_bytes(data.clone(), "probe", 400, false).ok()?;
            Some(data)
        })
    }

    fn write_fixture_font(label: &str) -> Option<(PathBuf, PathBuf)> {
        let data = fixture_font_bytes()?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "liquide-font-worker-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("fixture.ttf");
        std::fs::write(&path, data).ok()?;
        Some((dir, path))
    }

    /// Regression for t49-e3-F15: a stale-face invalidation must *reload* the
    /// font bytes from disk, not merely bump the worker generation. After
    /// `invalidate_stale_faces`, the shared database must report the face as
    /// fresh again (the source stamp was re-captured from the new bytes).
    #[test]
    fn invalidate_stale_faces_reloads_bytes_and_clears_staleness() {
        let Some((dir, path)) = write_fixture_font("reload") else {
            return;
        };
        let mut db = FontDatabase::new();
        let face_id = db.load_file(&path, "Fixture", 400, false).unwrap();
        let original_len = db.get(face_id).unwrap().raw_data.len();
        let mut worker = FontWorker::with_font_db(db);

        // Bump a generation so we can observe the invalidation side effect.
        let gen_before = worker.generation;
        assert!(worker.stale_faces().is_empty());

        // Mutate the source on disk.
        let mut data = std::fs::read(&path).unwrap();
        data.extend_from_slice(b"reload-trailer");
        let new_len = data.len();
        std::fs::write(&path, &data).unwrap();

        assert_eq!(worker.stale_faces(), vec![face_id]);

        let stale = worker.invalidate_stale_faces();
        assert_eq!(stale, vec![face_id]);

        // The DB now serves the FRESH bytes and the face is no longer stale.
        {
            let dbg = worker.font_db().lock().unwrap();
            assert_eq!(dbg.get(face_id).unwrap().raw_data.len(), new_len);
            assert_ne!(new_len, original_len);
            assert!(dbg.stale_faces().is_empty(), "reloaded face is fresh");
        }
        // Worker-side generation advanced so stale in-flight glyphs are dropped.
        assert_ne!(worker.generation, gen_before);
        assert_eq!(worker.pending_count(), 0);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `reload_faces` on a memory-loaded face is a no-op (no path) but still
    /// invalidates worker state for the requested ids without panicking.
    #[test]
    fn reload_faces_handles_memory_face_without_reload() {
        let Some(data) = fixture_font_bytes() else {
            return;
        };
        let mut db = FontDatabase::new();
        let mem_id = db.load_bytes(data, "Memory", 400, false).unwrap();
        let mut worker = FontWorker::with_font_db(db);

        let reloaded = worker.reload_faces([mem_id]);
        assert!(reloaded.is_empty(), "memory face has no source to reload");
    }

    /// Drain the worker (busy-polling with a bounded deadline) until glyphs for
    /// every key in `keys` have been produced. Returns them keyed by GlyphKey.
    /// Because `poll_results` consumes results, we must collect all wanted keys
    /// in a single drain loop rather than one key at a time.
    fn collect_glyphs(
        worker: &mut FontWorker,
        keys: &[GlyphKey],
    ) -> HashMap<GlyphKey, RasterizedGlyph> {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut found: HashMap<GlyphKey, RasterizedGlyph> = HashMap::new();
        while found.len() < keys.len() {
            for glyph in worker.poll_results() {
                if keys.contains(&glyph.key) {
                    found.insert(glyph.key, glyph);
                }
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::yield_now();
        }
        found
    }

    /// font-style: italic must render differently from regular. When only an
    /// upright face is loaded for a family, requesting an italic glyph must
    /// produce a *different* bitmap than the upright glyph (synthetic oblique
    /// slant applied). This is the end-to-end proof that `italic` is honoured
    /// through `request_glyph_with_font` → `db.resolve` → synthetic oblique.
    #[test]
    fn italic_run_renders_differently_from_regular() {
        let Some(data) = fixture_font_bytes() else {
            return; // No system font available in this environment.
        };
        // Load ONLY an upright face for the family, so italic must be synthesized.
        let mut db = FontDatabase::new();
        db.load_bytes(data, "ItalicProbe", 400, false).unwrap();
        let mut worker = FontWorker::with_font_db(db);

        let target_height = 32_u32;
        // font_id layout (see renderer::text): bit 24 = italic, bits 16..24 =
        // weight, bits 0..16 = family hash. Use weight 400 (0x190) + hash 1.
        let upright_key = GlyphKey {
            font_id: ((400_u32 & 0xFF) << 16) | 1, // matches renderer::text encoding
            glyph_id: 'a' as u32,
            size_px: target_height as u16,
            subpixel: false,
        };
        // Distinct key (italic bit set) so the results don't collide.
        let italic_key = GlyphKey {
            font_id: upright_key.font_id | (1 << 24),
            ..upright_key
        };
        assert_ne!(upright_key.font_id, italic_key.font_id);

        worker.request_glyph_with_font(
            upright_key,
            'a',
            target_height,
            "ItalicProbe".to_string(),
            400,
            false,
        );
        worker.request_glyph_with_font(
            italic_key,
            'a',
            target_height,
            "ItalicProbe".to_string(),
            400,
            true,
        );

        let mut glyphs = collect_glyphs(&mut worker, &[upright_key, italic_key]);
        let upright = glyphs.remove(&upright_key).expect("upright glyph rasterized");
        let italic = glyphs.remove(&italic_key).expect("italic glyph rasterized");

        // The synthetic-oblique shear widens the bitmap and shifts coverage, so
        // the two bitmaps must differ. (A real italic face would also differ.)
        let differs = upright.metrics.width != italic.metrics.width
            || upright.bitmap != italic.bitmap;
        assert!(
            differs,
            "italic glyph bitmap must differ from upright (synthetic oblique not applied): \
             upright {}x{} ({} bytes) vs italic {}x{} ({} bytes)",
            upright.metrics.width,
            upright.metrics.height,
            upright.bitmap.len(),
            italic.metrics.width,
            italic.metrics.height,
            italic.bitmap.len(),
        );

        // Synthetic oblique specifically widens the glyph (shear adds columns).
        assert!(
            italic.metrics.width >= upright.metrics.width,
            "synthetic oblique should not narrow the glyph"
        );
    }
}
