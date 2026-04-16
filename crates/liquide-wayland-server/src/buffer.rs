#[derive(Debug, Clone)]
pub enum BufferSource {
    Shm { pool_id: u32, offset: usize },
    DmaBuf { fd: i32, modifier: u64 },
    Null,
}

#[derive(Debug, Clone)]
pub struct BufferRef {
    pub source: BufferSource,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
}

impl BufferRef {
    pub fn new(source: BufferSource, width: u32, height: u32, stride: u32, format: u32) -> Self {
        Self {
            source,
            width,
            height,
            stride,
            format,
        }
    }

    pub fn null() -> Self {
        Self {
            source: BufferSource::Null,
            width: 0,
            height: 0,
            stride: 0,
            format: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.source, BufferSource::Null)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
