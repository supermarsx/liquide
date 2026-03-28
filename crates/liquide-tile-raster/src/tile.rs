//! Tile primitive: a fixed-size rectangle of RGBA pixels.

/// Default tile size in pixels (256x256).
pub const DEFAULT_TILE_SIZE: u32 = 256;

/// Grid position of a tile within the tile grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    /// Column index (x axis).
    pub col: u32,
    /// Row index (y axis).
    pub row: u32,
}

impl TileId {
    /// Create a new tile identifier.
    #[inline]
    pub fn new(col: u32, row: u32) -> Self {
        Self { col, row }
    }

    /// Manhattan distance from another tile (used for scheduling priority).
    #[inline]
    pub fn manhattan_distance(&self, other: &TileId) -> u32 {
        self.col.abs_diff(other.col) + self.row.abs_diff(other.row)
    }
}

/// Current state of a tile in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileState {
    /// Pixel data is up-to-date, no changes needed.
    Clean,
    /// Display list changed in this tile's region; needs re-rasterization.
    Dirty,
    /// Currently being rasterized (reserved for multi-threaded scheduling).
    Pending,
    /// Never been rendered (initial state for new tiles).
    Empty,
}

/// A single tile: a fixed-size rectangle of RGBA pixel data.
pub struct Tile {
    /// Grid position.
    pub id: TileId,
    /// Current state.
    pub state: TileState,
    /// RGBA pixel data (tile_size x tile_size x 4 bytes).
    /// For edge tiles this may be smaller (actual_width x actual_height x 4).
    pub pixels: Vec<u8>,
    /// Actual width in pixels (may be less than tile_size at the right edge).
    pub width: u32,
    /// Actual height in pixels (may be less than tile_size at the bottom edge).
    pub height: u32,
    /// Generation counter: incremented each time the tile is re-rasterized.
    /// Used by the compositor to know which tiles need blitting.
    pub generation: u64,
}

impl Tile {
    /// Create a new empty tile.
    pub fn new(id: TileId, width: u32, height: u32) -> Self {
        let pixel_count = (width as usize) * (height as usize) * 4;
        Self {
            id,
            state: TileState::Empty,
            pixels: vec![0u8; pixel_count],
            width,
            height,
            generation: 0,
        }
    }

    /// Byte length of the pixel data.
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }

    /// Stride in bytes (width * 4).
    #[inline]
    pub fn stride(&self) -> usize {
        self.width as usize * 4
    }

    /// Clear the tile to transparent black.
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// Clear the tile to a specific RGBA color.
    pub fn clear_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    /// Get the pixel-space origin (top-left corner) of this tile.
    #[inline]
    pub fn origin_x(&self, tile_size: u32) -> u32 {
        self.id.col * tile_size
    }

    /// Get the pixel-space origin y of this tile.
    #[inline]
    pub fn origin_y(&self, tile_size: u32) -> u32 {
        self.id.row * tile_size
    }

    /// Check if the tile has valid pixel data.
    #[inline]
    pub fn is_renderable(&self) -> bool {
        self.state == TileState::Clean && !self.pixels.is_empty()
    }
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tile")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("generation", &self.generation)
            .field("pixel_bytes", &self.pixels.len())
            .finish()
    }
}

/// Validate that a tile size is one of the supported values.
pub fn validate_tile_size(size: u32) -> bool {
    matches!(size, 128 | 256 | 512)
}
