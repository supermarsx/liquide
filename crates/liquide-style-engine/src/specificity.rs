//! CSS specificity calculation.

/// CSS specificity as `(id_count, class_count, type_count)`.
///
/// Comparison follows CSS spec: leftmost column wins, ties go to next column.
/// Within the same specificity, source order (later wins) is handled by the
/// engine, not by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Specificity {
    /// Number of ID selectors.
    pub id: u32,
    /// Number of class selectors, pseudo-classes, and attribute selectors.
    pub class: u32,
    /// Number of type selectors and pseudo-elements.
    pub type_sel: u32,
}

impl Specificity {
    pub const ZERO: Specificity = Specificity {
        id: 0,
        class: 0,
        type_sel: 0,
    };

    pub fn new(id: u32, class: u32, type_sel: u32) -> Self {
        Self {
            id,
            class,
            type_sel,
        }
    }

    /// Combine specificities (for nested selectors).
    pub fn add(self, other: Specificity) -> Specificity {
        Specificity {
            id: self.id + other.id,
            class: self.class + other.class,
            type_sel: self.type_sel + other.type_sel,
        }
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id
            .cmp(&other.id)
            .then(self.class.cmp(&other.class))
            .then(self.type_sel.cmp(&other.type_sel))
    }
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering() {
        let a = Specificity::new(1, 0, 0); // #id
        let b = Specificity::new(0, 10, 10); // .c1.c2...c10 p...p10
        assert!(a > b, "ID selector should beat any number of classes");
    }

    #[test]
    fn equal() {
        let a = Specificity::new(0, 1, 1);
        let b = Specificity::new(0, 1, 1);
        assert_eq!(a, b);
    }

    #[test]
    fn add() {
        let a = Specificity::new(1, 2, 0);
        let b = Specificity::new(0, 1, 3);
        assert_eq!(a.add(b), Specificity::new(1, 3, 3));
    }
}
