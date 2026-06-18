//! Tile URL templating + the stable tile KEY used to bind a fetched tile to its
//! on-screen `Image` node.
//!
//! A tile-server URL is a template with `{z}`/`{x}`/`{y}` placeholders. The
//! default is the OpenStreetMap standard tile server. (OSM's tile-usage policy
//! requires a descriptive User-Agent — that is set by `liquide-http`'s
//! `HttpConfig::user_agent` — and discourages heavy automated use; this map
//! element fetches only the handful of tiles a viewport needs and caches them.)

use crate::slippy::TileId;

/// The standard OpenStreetMap tile server URL template.
pub const DEFAULT_OSM_TEMPLATE: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";

/// Fill a `{z}/{x}/{y}` URL template for a (canonical) tile id.
///
/// The id is canonicalised (x wrapped into range) so the URL is always valid for
/// a date-line-straddling viewport.
#[must_use]
pub fn tile_url(template: &str, id: TileId) -> String {
    let id = id.canonical();
    template
        .replace("{z}", &id.z.to_string())
        .replace("{x}", &id.x.to_string())
        .replace("{y}", &id.y.to_string())
}

/// A STABLE, opaque string key identifying a tile's image content, independent
/// of the tile-server URL. The shell uses this as the `background-image` src of
/// the tile's DOM element; the scene bridge hashes it to the renderer image id,
/// and the session decode path registers the decoded RGBA under the SAME hash —
/// so the tile's `Image` scene node paints the right bytes.
///
/// Form: `tile://{z}/{x}/{y}` over the canonical (wrapped) id, so the same world
/// tile reached by panning around the globe maps to ONE image (de-dupe).
#[must_use]
pub fn tile_image_key(id: TileId) -> String {
    let id = id.canonical();
    format!("tile://{}/{}/{}", id.z, id.x, id.y)
}

/// Parse a [`tile_image_key`] string back into a [`TileId`], or `None` if it is
/// not a well-formed `tile://z/x/y` key. Used by the session render path to map a
/// decoded tile's image-key back to its address.
#[must_use]
pub fn parse_tile_image_key(key: &str) -> Option<TileId> {
    let rest = key.strip_prefix("tile://")?;
    let mut parts = rest.split('/');
    let z = parts.next()?.parse::<u32>().ok()?;
    let x = parts.next()?.parse::<i64>().ok()?;
    let y = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(TileId::new(z, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_fills_the_template() {
        let url = tile_url(DEFAULT_OSM_TEMPLATE, TileId::new(12, 2200, 1343));
        assert_eq!(url, "https://tile.openstreetmap.org/12/2200/1343.png");
    }

    #[test]
    fn url_canonicalises_a_wrapped_x() {
        // x=-1 at z=2 wraps to 3.
        let url = tile_url("h/{z}/{x}/{y}", TileId::new(2, -1, 1));
        assert_eq!(url, "h/2/3/1");
    }

    #[test]
    fn image_key_round_trips() {
        let id = TileId::new(9, 271, 195);
        let key = tile_image_key(id);
        assert_eq!(key, "tile://9/271/195");
        assert_eq!(parse_tile_image_key(&key), Some(id));
        // Garbage / wrong shape → None (never panics).
        assert_eq!(parse_tile_image_key("nope"), None);
        assert_eq!(parse_tile_image_key("tile://9/271"), None);
        assert_eq!(parse_tile_image_key("tile://9/271/195/extra"), None);
    }

    #[test]
    fn image_key_canonicalises_so_wrapped_tiles_dedupe() {
        // The same world tile reached via a wrapped x maps to one image key.
        let a = tile_image_key(TileId::new(2, 4, 1)); // wraps to 0
        let b = tile_image_key(TileId::new(2, 0, 1));
        assert_eq!(a, b);
    }
}
