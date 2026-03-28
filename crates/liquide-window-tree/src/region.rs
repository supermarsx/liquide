use crate::Rect;

/// A clipping / invalidation region composed of rectangles.
///
/// This is intentionally simpler than a full GDI HRGN — it handles the common
/// cases (empty, single rect, small rect list) efficiently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// No area.
    Empty,
    /// Single rectangle.
    Rect(Rect),
    /// Union of non-overlapping rectangles (band-sorted is not required).
    RectList(Vec<Rect>),
}

impl Default for Region {
    fn default() -> Self {
        Region::Empty
    }
}

impl Region {
    /// Test if a point falls inside the region.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        match self {
            Region::Empty => false,
            Region::Rect(r) => r.contains_point(x, y),
            Region::RectList(rects) => rects.iter().any(|r| r.contains_point(x, y)),
        }
    }

    /// Whether the region represents no area.
    pub fn is_empty(&self) -> bool {
        match self {
            Region::Empty => true,
            Region::Rect(r) => r.is_empty(),
            Region::RectList(rects) => rects.is_empty() || rects.iter().all(|r| r.is_empty()),
        }
    }

    /// Compute the bounding rectangle of the region.
    pub fn bounding_rect(&self) -> Option<Rect> {
        match self {
            Region::Empty => None,
            Region::Rect(r) if r.is_empty() => None,
            Region::Rect(r) => Some(*r),
            Region::RectList(rects) => {
                let mut result: Option<Rect> = None;
                for r in rects {
                    if r.is_empty() {
                        continue;
                    }
                    result = Some(match result {
                        Some(acc) => acc.union(r),
                        None => *r,
                    });
                }
                result
            }
        }
    }

    /// Intersect this region with another region.
    pub fn intersect(&self, other: &Region) -> Region {
        match (self, other) {
            (Region::Empty, _) | (_, Region::Empty) => Region::Empty,
            (Region::Rect(a), Region::Rect(b)) => {
                let i = a.intersection(b);
                if i.is_empty() { Region::Empty } else { Region::Rect(i) }
            }
            (Region::Rect(a), Region::RectList(bs)) | (Region::RectList(bs), Region::Rect(a)) => {
                let result: Vec<Rect> = bs.iter()
                    .map(|b| a.intersection(b))
                    .filter(|r| !r.is_empty())
                    .collect();
                Self::from_rects(result)
            }
            (Region::RectList(a_list), Region::RectList(b_list)) => {
                let mut result = Vec::new();
                for a in a_list {
                    for b in b_list {
                        let i = a.intersection(b);
                        if !i.is_empty() {
                            result.push(i);
                        }
                    }
                }
                Self::from_rects(result)
            }
        }
    }

    /// Union this region with another region.
    pub fn union(&self, other: &Region) -> Region {
        match (self, other) {
            (Region::Empty, r) => r.clone(),
            (r, Region::Empty) => r.clone(),
            (Region::Rect(a), Region::Rect(b)) => {
                Region::RectList(vec![*a, *b])
            }
            (Region::Rect(a), Region::RectList(bs)) => {
                let mut result = vec![*a];
                result.extend_from_slice(bs);
                Region::RectList(result)
            }
            (Region::RectList(a_list), Region::Rect(b)) => {
                let mut result = a_list.clone();
                result.push(*b);
                Region::RectList(result)
            }
            (Region::RectList(a_list), Region::RectList(b_list)) => {
                let mut result = a_list.clone();
                result.extend_from_slice(b_list);
                Region::RectList(result)
            }
        }
    }

    /// Subtract another region from this region.
    ///
    /// For `Rect - Rect`, produces up to 4 strips. For more complex cases,
    /// subtracts each rect in `other` from each rect in `self`.
    pub fn subtract(&self, other: &Region) -> Region {
        match (self, other) {
            (Region::Empty, _) => Region::Empty,
            (r, Region::Empty) => r.clone(),
            (Region::Rect(a), Region::Rect(b)) => {
                Self::from_rects(subtract_rect_rect(a, b))
            }
            (Region::Rect(a), Region::RectList(bs)) => {
                let mut current = vec![*a];
                for b in bs {
                    let mut next = Vec::new();
                    for c in &current {
                        next.extend(subtract_rect_rect(c, b));
                    }
                    current = next;
                }
                Self::from_rects(current)
            }
            (Region::RectList(a_list), _) => {
                let mut result = Vec::new();
                for a in a_list {
                    let sub = Region::Rect(*a).subtract(other);
                    match sub {
                        Region::Empty => {}
                        Region::Rect(r) => result.push(r),
                        Region::RectList(rs) => result.extend(rs),
                    }
                }
                Self::from_rects(result)
            }
        }
    }

    /// Normalize a rect vec into a region variant.
    fn from_rects(rects: Vec<Rect>) -> Region {
        let nonempty: Vec<Rect> = rects.into_iter().filter(|r| !r.is_empty()).collect();
        match nonempty.len() {
            0 => Region::Empty,
            1 => Region::Rect(nonempty[0]),
            _ => Region::RectList(nonempty),
        }
    }
}

/// Subtract rectangle `b` from rectangle `a`, producing up to 4 strips.
///
/// ```text
///   +--------+           +--------+
///   |   a    |           |  top   |
///   |  +--+  |   =>      +--+--+--+
///   |  | b|  |           |L |  |R |
///   |  +--+  |           +--+--+--+
///   |        |           | bottom |
///   +--------+           +--------+
/// ```
fn subtract_rect_rect(a: &Rect, b: &Rect) -> Vec<Rect> {
    let i = a.intersection(b);
    if i.is_empty() {
        return vec![*a];
    }

    let mut result = Vec::with_capacity(4);

    // Top strip
    if i.y > a.y {
        result.push(Rect::new(a.x, a.y, a.width, i.y - a.y));
    }
    // Bottom strip
    if i.bottom() < a.bottom() {
        result.push(Rect::new(a.x, i.bottom(), a.width, a.bottom() - i.bottom()));
    }
    // Left strip (between top and bottom)
    if i.x > a.x {
        result.push(Rect::new(a.x, i.y, i.x - a.x, i.height));
    }
    // Right strip (between top and bottom)
    if i.right() < a.right() {
        result.push(Rect::new(i.right(), i.y, a.right() - i.right(), i.height));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_region() {
        let r = Region::Empty;
        assert!(r.is_empty());
        assert!(!r.contains_point(0, 0));
        assert_eq!(r.bounding_rect(), None);
    }

    #[test]
    fn rect_region_contains() {
        let r = Region::Rect(Rect::new(10, 10, 50, 50));
        assert!(r.contains_point(20, 20));
        assert!(!r.contains_point(5, 5));
        assert!(!r.is_empty());
        assert_eq!(r.bounding_rect(), Some(Rect::new(10, 10, 50, 50)));
    }

    #[test]
    fn rect_list_region() {
        let r = Region::RectList(vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(20, 20, 10, 10),
        ]);
        assert!(r.contains_point(5, 5));
        assert!(r.contains_point(25, 25));
        assert!(!r.contains_point(15, 15));
        assert_eq!(r.bounding_rect(), Some(Rect::new(0, 0, 30, 30)));
    }

    #[test]
    fn region_intersect() {
        let a = Region::Rect(Rect::new(0, 0, 100, 100));
        let b = Region::Rect(Rect::new(50, 50, 100, 100));
        let i = a.intersect(&b);
        assert_eq!(i, Region::Rect(Rect::new(50, 50, 50, 50)));
    }

    #[test]
    fn region_intersect_empty() {
        let a = Region::Rect(Rect::new(0, 0, 10, 10));
        let b = Region::Rect(Rect::new(100, 100, 10, 10));
        assert_eq!(a.intersect(&b), Region::Empty);
    }

    #[test]
    fn region_union() {
        let a = Region::Rect(Rect::new(0, 0, 10, 10));
        let b = Region::Rect(Rect::new(20, 20, 10, 10));
        let u = a.union(&b);
        match u {
            Region::RectList(ref rects) => assert_eq!(rects.len(), 2),
            _ => panic!("expected RectList"),
        }
        assert!(u.contains_point(5, 5));
        assert!(u.contains_point(25, 25));
    }

    #[test]
    fn region_subtract_no_overlap() {
        let a = Region::Rect(Rect::new(0, 0, 10, 10));
        let b = Region::Rect(Rect::new(100, 100, 10, 10));
        let s = a.subtract(&b);
        assert_eq!(s, Region::Rect(Rect::new(0, 0, 10, 10)));
    }

    #[test]
    fn region_subtract_full_cover() {
        let a = Region::Rect(Rect::new(10, 10, 20, 20));
        let b = Region::Rect(Rect::new(0, 0, 100, 100));
        let s = a.subtract(&b);
        assert!(s.is_empty());
    }

    #[test]
    fn region_subtract_partial() {
        let a = Region::Rect(Rect::new(0, 0, 100, 100));
        let b = Region::Rect(Rect::new(25, 25, 50, 50));
        let s = a.subtract(&b);
        // Should produce 4 strips
        match s {
            Region::RectList(ref rects) => {
                assert_eq!(rects.len(), 4);
                // Total area should be 100*100 - 50*50 = 7500
                let total_area: i64 = rects.iter().map(|r| r.area()).sum();
                assert_eq!(total_area, 7500);
            }
            _ => panic!("expected RectList, got {:?}", s),
        }
    }

    #[test]
    fn region_intersect_rect_list() {
        let a = Region::RectList(vec![
            Rect::new(0, 0, 50, 50),
            Rect::new(60, 60, 50, 50),
        ]);
        let b = Region::Rect(Rect::new(25, 25, 50, 50));
        let i = a.intersect(&b);
        // First rect intersects at (25,25,25,25), second at (60,60,15,15)
        match i {
            Region::RectList(ref rects) => {
                assert_eq!(rects.len(), 2);
                assert!(rects.contains(&Rect::new(25, 25, 25, 25)));
                assert!(rects.contains(&Rect::new(60, 60, 15, 15)));
            }
            _ => panic!("expected RectList"),
        }
    }
}
