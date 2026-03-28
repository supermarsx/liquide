//! Tile grid: manages a 2D grid of tiles covering the viewport.

use crate::tile::{Tile, TileId, TileState};

/// A rectangle in pixel coordinates (f32) used for damage and clipping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PixelRect {
    /// Create a new pixel rectangle.
    #[inline]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Right edge.
    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Check intersection with another rect.
    #[inline]
    pub fn intersects(&self, other: &PixelRect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Compute the intersection.
    #[inline]
    pub fn intersection(&self, other: &PixelRect) -> Option<PixelRect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right > x && bottom > y {
            Some(PixelRect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Union of two rects.
    #[inline]
    pub fn union(&self, other: &PixelRect) -> PixelRect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        PixelRect::new(x, y, right - x, bottom - y)
    }

    /// Area in square pixels.
    #[inline]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if area is zero or negative.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// A 2D grid of tiles covering the entire viewport.
pub struct TileGrid {
    /// Tiles stored row-major: tiles[row * cols + col].
    tiles: Vec<Tile>,
    /// Number of tile columns.
    cols: u32,
    /// Number of tile rows.
    rows: u32,
    /// Tile size in pixels (one of 128, 256, 512).
    tile_size: u32,
    /// Viewport width in pixels.
    viewport_width: u32,
    /// Viewport height in pixels.
    viewport_height: u32,
}

impl TileGrid {
    /// Create a new tile grid covering the given viewport.
    pub fn new(viewport_width: u32, viewport_height: u32, tile_size: u32) -> Self {
        let cols = viewport_width.div_ceil(tile_size);
        let rows = viewport_height.div_ceil(tile_size);

        let mut tiles = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let tw = tile_size.min(viewport_width.saturating_sub(col * tile_size));
                let th = tile_size.min(viewport_height.saturating_sub(row * tile_size));
                tiles.push(Tile::new(TileId::new(col, row), tw, th));
            }
        }

        Self {
            tiles,
            cols,
            rows,
            tile_size,
            viewport_width,
            viewport_height,
        }
    }

    /// Number of tile columns.
    #[inline]
    pub fn cols(&self) -> u32 {
        self.cols
    }

    /// Number of tile rows.
    #[inline]
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// Tile size in pixels.
    #[inline]
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    /// Viewport width.
    #[inline]
    pub fn viewport_width(&self) -> u32 {
        self.viewport_width
    }

    /// Viewport height.
    #[inline]
    pub fn viewport_height(&self) -> u32 {
        self.viewport_height
    }

    /// Total number of tiles in the grid.
    #[inline]
    pub fn tile_count(&self) -> u32 {
        self.cols * self.rows
    }

    /// Get a reference to the tile at (col, row).
    ///
    /// # Panics
    /// Panics if col >= cols or row >= rows.
    #[inline]
    pub fn tile_at(&self, col: u32, row: u32) -> &Tile {
        debug_assert!(col < self.cols && row < self.rows,
            "tile_at({col}, {row}) out of bounds ({}, {})", self.cols, self.rows);
        &self.tiles[(row * self.cols + col) as usize]
    }

    /// Get a mutable reference to the tile at (col, row).
    ///
    /// # Panics
    /// Panics if col >= cols or row >= rows.
    #[inline]
    pub fn tile_at_mut(&mut self, col: u32, row: u32) -> &mut Tile {
        debug_assert!(col < self.cols && row < self.rows,
            "tile_at_mut({col}, {row}) out of bounds ({}, {})", self.cols, self.rows);
        &mut self.tiles[(row * self.cols + col) as usize]
    }

    /// Get the tile ID for a pixel coordinate.
    #[inline]
    pub fn tile_for_point(&self, x: u32, y: u32) -> TileId {
        TileId::new(
            (x / self.tile_size).min(self.cols.saturating_sub(1)),
            (y / self.tile_size).min(self.rows.saturating_sub(1)),
        )
    }

    /// Get all tile IDs intersecting a pixel-space rectangle.
    pub fn tiles_for_rect(&self, rect: &PixelRect) -> Vec<TileId> {
        if rect.is_empty() {
            return Vec::new();
        }

        let ts = self.tile_size as f32;
        let col_start = (rect.x / ts).floor().max(0.0) as u32;
        let row_start = (rect.y / ts).floor().max(0.0) as u32;
        let col_end = (rect.right() / ts).ceil().min(self.cols as f32) as u32;
        let row_end = (rect.bottom() / ts).ceil().min(self.rows as f32) as u32;

        let mut result = Vec::with_capacity(
            ((col_end - col_start) * (row_end - row_start)) as usize,
        );
        for row in row_start..row_end {
            for col in col_start..col_end {
                result.push(TileId::new(col, row));
            }
        }
        result
    }

    /// Resize the grid for a new viewport size, preserving clean tiles that still fit.
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width == self.viewport_width && new_height == self.viewport_height {
            return;
        }

        let new_cols = new_width.div_ceil(self.tile_size);
        let new_rows = new_height.div_ceil(self.tile_size);

        let mut new_tiles = Vec::with_capacity((new_cols * new_rows) as usize);
        for row in 0..new_rows {
            for col in 0..new_cols {
                let tw = self.tile_size.min(new_width.saturating_sub(col * self.tile_size));
                let th = self.tile_size.min(new_height.saturating_sub(row * self.tile_size));

                // Try to reuse existing clean tile if it fits
                if col < self.cols && row < self.rows {
                    let old_idx = (row * self.cols + col) as usize;
                    let old_tile = &self.tiles[old_idx];
                    if old_tile.width == tw && old_tile.height == th
                        && old_tile.state == TileState::Clean
                    {
                        // Preserve the clean tile
                        let mut tile = Tile::new(TileId::new(col, row), tw, th);
                        tile.pixels = old_tile.pixels.clone();
                        tile.state = TileState::Clean;
                        tile.generation = old_tile.generation;
                        new_tiles.push(tile);
                        continue;
                    }
                }

                new_tiles.push(Tile::new(TileId::new(col, row), tw, th));
            }
        }

        self.tiles = new_tiles;
        self.cols = new_cols;
        self.rows = new_rows;
        self.viewport_width = new_width;
        self.viewport_height = new_height;
    }

    /// Mark all tiles touching the given rect as Dirty.
    pub fn invalidate_rect(&mut self, rect: &PixelRect) {
        let tile_ids = self.tiles_for_rect(rect);
        for id in tile_ids {
            let tile = self.tile_at_mut(id.col, id.row);
            if tile.state == TileState::Clean || tile.state == TileState::Empty {
                tile.state = TileState::Dirty;
            }
        }
    }

    /// Mark all tiles in the grid as Dirty.
    pub fn invalidate_all(&mut self) {
        for tile in &mut self.tiles {
            tile.state = TileState::Dirty;
        }
    }

    /// Return all tile IDs that are in the Dirty state.
    pub fn dirty_tiles(&self) -> Vec<TileId> {
        self.tiles
            .iter()
            .filter(|t| t.state == TileState::Dirty)
            .map(|t| t.id)
            .collect()
    }

    /// Return all tile IDs that need rasterization (Dirty or Empty).
    pub fn pending_tiles(&self) -> Vec<TileId> {
        self.tiles
            .iter()
            .filter(|t| t.state == TileState::Dirty || t.state == TileState::Empty)
            .map(|t| t.id)
            .collect()
    }

    /// Mark a tile as Clean after rasterization.
    pub fn clean_tile(&mut self, id: TileId) {
        if id.col < self.cols && id.row < self.rows {
            self.tile_at_mut(id.col, id.row).state = TileState::Clean;
        }
    }

    /// Get the pixel-space bounding rect for a tile.
    pub fn tile_bounds(&self, id: TileId) -> PixelRect {
        let tile = self.tile_at(id.col, id.row);
        PixelRect::new(
            (id.col * self.tile_size) as f32,
            (id.row * self.tile_size) as f32,
            tile.width as f32,
            tile.height as f32,
        )
    }

    /// Iterate over all tiles.
    pub fn iter(&self) -> impl Iterator<Item = &Tile> {
        self.tiles.iter()
    }

    /// Iterate mutably over all tiles.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Tile> {
        self.tiles.iter_mut()
    }

    /// Count tiles in each state.
    pub fn state_counts(&self) -> TileStateCounts {
        let mut counts = TileStateCounts::default();
        for tile in &self.tiles {
            match tile.state {
                TileState::Clean => counts.clean += 1,
                TileState::Dirty => counts.dirty += 1,
                TileState::Pending => counts.pending += 1,
                TileState::Empty => counts.empty += 1,
            }
        }
        counts
    }
}

impl std::fmt::Debug for TileGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let counts = self.state_counts();
        f.debug_struct("TileGrid")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .field("tile_size", &self.tile_size)
            .field("viewport", &(self.viewport_width, self.viewport_height))
            .field("clean", &counts.clean)
            .field("dirty", &counts.dirty)
            .field("pending", &counts.pending)
            .field("empty", &counts.empty)
            .finish()
    }
}

/// Count of tiles in each state (for diagnostics).
#[derive(Debug, Default, Clone, Copy)]
pub struct TileStateCounts {
    pub clean: u32,
    pub dirty: u32,
    pub pending: u32,
    pub empty: u32,
}
