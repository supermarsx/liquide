//! Frame assembler — applies tile batches to reconstruct full frames.

use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::tile::{TileBatch, TileConfig, TileEncoding};

use crate::decoder::TileDecoder;
use crate::presenter::Presenter;
use crate::surface::RenderSurface;

/// Result of applying a single tile batch to the frame assembler.
#[derive(Debug, Clone)]
pub struct FrameResult {
    /// Number of tiles that were decoded (non-skip).
    pub tiles_decoded: u32,
    /// Number of tiles that were skipped.
    pub tiles_skipped: u32,
    /// Total bytes decompressed across all tiles.
    pub bytes_decompressed: u64,
    /// Total decode time in microseconds.
    pub decode_time_us: u64,
}

impl FrameResult {
    /// Total tiles processed (decoded + skipped).
    #[must_use]
    pub fn total_tiles(&self) -> u32 {
        self.tiles_decoded + self.tiles_skipped
    }
}

impl std::fmt::Display for FrameResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FrameResult(decoded={}, skipped={}, bytes={}, time={}us)",
            self.tiles_decoded, self.tiles_skipped, self.bytes_decompressed, self.decode_time_us
        )
    }
}

/// Assembles decoded tiles into a complete frame.
///
/// Combines a [`RenderSurface`] and [`TileDecoder`] to process
/// incoming [`TileBatch`] updates from the encoder pipeline.
pub struct FrameAssembler {
    surface: RenderSurface,
    decoder: TileDecoder,
    frame_count: u64,
    tile_size: u32,
}

impl FrameAssembler {
    /// Create a new frame assembler for the given dimensions and tile config.
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat, config: TileConfig) -> Self {
        let tile_size = config.tile_size;
        let cols = width.div_ceil(tile_size);
        let rows = height.div_ceil(tile_size);
        let surface = RenderSurface::new(width, height, format);
        let decoder = TileDecoder::new(cols, rows, config);
        Self {
            surface,
            decoder,
            frame_count: 0,
            tile_size,
        }
    }

    /// Apply a tile batch to the frame, decoding and writing each tile.
    pub fn apply_batch(&mut self, batch: &TileBatch) -> crate::Result<FrameResult> {
        let start = std::time::Instant::now();
        let mut tiles_decoded = 0u32;
        let mut tiles_skipped = 0u32;
        let mut bytes_decompressed = 0u64;

        // Phase 1: Decode all tiles (no state mutation).
        // If any tile fails to decode, we return early without committing
        // partial results, keeping the decoder in a consistent state.
        let mut decoded = Vec::with_capacity(batch.tiles.len());
        for update in &batch.tiles {
            let data = self.decoder.decode_tile(update)?;

            if update.encoding == TileEncoding::Skip {
                tiles_skipped += 1;
            } else {
                tiles_decoded += 1;
                bytes_decompressed += data.len() as u64;
            }

            decoded.push((update.tx, update.ty, data));
        }

        // Phase 2: Commit all decoded tiles and write to surface.
        // Only reached if all tiles decoded successfully.
        for (tx, ty, data) in decoded {
            if !self.surface.write_tile(tx, ty, self.tile_size, &data) {
                // Incomplete write (usually buffer-size mismatch at the
                // frame edge + truncated tile). Return a recoverable error
                // so the caller can request reassembly retry on the next
                // batch rather than silently committing corrupted tiles.
                return Err(crate::ClientRendererError::IncompleteTile { tx, ty });
            }
            self.decoder.commit_tile(tx, ty, data);
        }

        self.frame_count += 1;

        Ok(FrameResult {
            tiles_decoded,
            tiles_skipped,
            bytes_decompressed,
            decode_time_us: start.elapsed().as_micros() as u64,
        })
    }

    /// Reference to the current surface.
    #[must_use]
    pub fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    /// Mutable reference to the current surface.
    pub fn surface_mut(&mut self) -> &mut RenderSurface {
        &mut self.surface
    }

    /// Present the current surface using the supplied presenter.
    pub fn present<P: Presenter>(&self, presenter: &mut P) -> crate::Result<()> {
        if !presenter.supports_format(self.surface.format()) {
            return Err(crate::ClientRendererError::PresenterError(format!(
                "presenter does not support pixel format {}",
                self.surface.format().wire_name()
            )));
        }
        presenter.present(&self.surface)
    }

    /// Reference to the tile decoder.
    #[must_use]
    pub fn decoder(&self) -> &TileDecoder {
        &self.decoder
    }

    /// Number of frames processed so far.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Resize the assembler for new dimensions, clearing all state.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(width, height);
        let cols = width.div_ceil(self.tile_size);
        let rows = height.div_ceil(self.tile_size);
        self.decoder.resize(cols, rows);
        self.frame_count = 0;
    }

    /// Reset all state (surface, decoder, frame count).
    pub fn reset(&mut self) {
        self.surface.clear();
        self.decoder.reset();
        self.frame_count = 0;
    }
}

impl std::fmt::Display for FrameAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FrameAssembler({}x{}, frames={})",
            self.surface.width(),
            self.surface.height(),
            self.frame_count
        )
    }
}
