//! CSS selector model with full combinator support.
//!
//! A complex selector like `div.container > p.intro:first-child` is:
//!
//! ```text
//! ComplexSelector {
//!   compounds: [
//!     CompoundSelector { tag: "div", classes: ["container"], .. },
//!     CompoundSelector { tag: "p", classes: ["intro"], pseudo: [FirstChild], .. },
//!   ],
//!   combinators: [Child],
//! }
//! ```

use liquide_dom::{Document, Node, NodeId, PseudoStateFlags};

use crate::specificity::Specificity;

/// How two compound selectors in a complex selector relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// `A B` — descendant.
    Descendant,
    /// `A > B` — direct child.
    Child,
    /// `A + B` — next sibling.
    NextSibling,
    /// `A ~ B` — subsequent sibling.
    SubsequentSibling,
}

/// A pseudo-class in a selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoClassSelector {
    Hover,
    Focus,
    Active,
    Visited,
    Disabled,
    Checked,
    FirstChild,
    LastChild,
    NthChild(AnB),
    NthLastChild(AnB),
    Not(Vec<ComplexSelector>),
    FocusWithin,
    FocusVisible,
    PlaceholderShown,
    ReadOnly,
    ReadWrite,
    Root,
    Empty,
    Is(Vec<ComplexSelector>),
    Where(Vec<ComplexSelector>),
    Has(Vec<ComplexSelector>),
    NthOfType(AnB),
    NthLastOfType(AnB),
    LastOfType,
    OnlyChild,
    OnlyOfType,
    /// `:target` — element is the URL fragment target.
    Target,
    /// `:scope` — element is the scoping root.
    Scope,
    /// `:lang(code)` — element language matches.
    Lang(String),
    /// `:first-of-type`
    FirstOfType,
    /// `:enabled`
    Enabled,
    /// `:default`
    Default,
    /// `:indeterminate`
    Indeterminate,
    /// `:required`
    Required,
    /// `:optional`
    Optional,
    /// `:valid`
    Valid,
    /// `:invalid`
    Invalid,
    /// `:in-range`
    InRange,
    /// `:out-of-range`
    OutOfRange,
    /// `:link` — unvisited hyperlink
    Link,
    /// `:any-link` — any hyperlink
    AnyLink,
    /// `:dir(ltr|rtl)` — directionality
    Dir(String),
    /// `:autofill` — auto-filled form element
    Autofill,
    /// `:modal` — modal dialog element
    Modal,
    /// `:fullscreen` — fullscreen element
    Fullscreen,
}

/// A CSS pseudo-element (e.g. `::before`, `::after`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
    FirstLine,
    FirstLetter,
    Placeholder,
    Selection,
}

/// `:nth-child(an+b)` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnB {
    pub a: i32,
    pub b: i32,
}

impl AnB {
    /// Check if a 1-based index matches `an+b`.
    pub fn matches(&self, index: i32) -> bool {
        if self.a == 0 {
            return index == self.b;
        }
        let diff = index - self.b;
        if self.a > 0 && diff < 0 {
            return false;
        }
        if self.a < 0 && diff > 0 {
            return false;
        }
        diff % self.a == 0
    }
}

/// Attribute selector operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeOp {
    /// `[attr]` — has attribute.
    Exists,
    /// `[attr=val]` — exact match.
    Equals(String),
    /// `[attr~=val]` — whitespace-separated word.
    Contains(String),
    /// `[attr|=val]` — exact or prefix with hyphen.
    DashMatch(String),
    /// `[attr^=val]` — starts with.
    Prefix(String),
    /// `[attr$=val]` — ends with.
    Suffix(String),
    /// `[attr*=val]` — substring.
    Substring(String),
}

/// An attribute selector like `[type="submit"]` or `[type="submit" i]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub op: AttributeOp,
    /// Case-insensitive matching (the `i` flag in CSS).
    pub case_insensitive: bool,
}

/// A single compound selector (within a complex selector).
///
/// Matches a single element by its tag/id/classes/pseudo-states/attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundSelector {
    /// Tag name filter (None = universal `*`).
    pub tag: Option<String>,
    /// ID filter.
    pub id: Option<String>,
    /// Class filters.
    pub classes: Vec<String>,
    /// Pseudo-class filters.
    pub pseudo_classes: Vec<PseudoClassSelector>,
    /// Attribute filters.
    pub attributes: Vec<AttributeSelector>,
    /// Optional pseudo-element (e.g. `::before`).
    pub pseudo_element: Option<PseudoElement>,
}

impl CompoundSelector {
    pub fn new() -> Self {
        Self {
            tag: None,
            id: None,
            classes: Vec::new(),
            pseudo_classes: Vec::new(),
            attributes: Vec::new(),
            pseudo_element: None,
        }
    }

    /// Calculate the specificity contribution of this compound.
    pub fn specificity(&self) -> Specificity {
        let id = if self.id.is_some() { 1 } else { 0 };
        let mut class = self.classes.len() as u32
            + self.attributes.len() as u32;
        let mut extra = Specificity::ZERO;
        for pc in &self.pseudo_classes {
            match pc {
                PseudoClassSelector::Not(inner) => {
                    // :not() adds the specificity of its most specific argument
                    if let Some(max_spec) = inner.iter().map(|s| s.specificity()).max() {
                        extra = extra.add(max_spec);
                    }
                }
                PseudoClassSelector::Is(selectors) => {
                    // :is() adds the specificity of its most specific argument
                    if let Some(max_spec) = selectors.iter().map(|s| s.specificity()).max() {
                        extra = extra.add(max_spec);
                    }
                }
                PseudoClassSelector::Where(_) => {
                    // :where() contributes zero specificity
                }
                PseudoClassSelector::Has(selectors) => {
                    if let Some(max_spec) = selectors.iter().map(|s| s.specificity()).max() {
                        extra = extra.add(max_spec);
                    }
                }
                _ => class += 1,
            }
        }
        let mut type_sel = if self.tag.is_some() { 1 } else { 0 };
        if self.pseudo_element.is_some() {
            type_sel += 1;
        }
        Specificity::new(id, class, type_sel).add(extra)
    }

    /// Check if this compound matches a given DOM node.
    pub fn matches_node(&self, node: &Node, doc: &Document) -> bool {
        // Tag
        if let Some(ref tag_name) = self.tag {
            if node.tag_name() != *tag_name {
                return false;
            }
        }

        // ID
        if let Some(ref sel_id) = self.id {
            match &node.element_id {
                Some(eid) if eid == sel_id => {}
                _ => return false,
            }
        }

        // Classes
        for cls in &self.classes {
            if !node.has_class(cls) {
                return false;
            }
        }

        // Pseudo-classes
        for pc in &self.pseudo_classes {
            if !self.matches_pseudo_class(pc, node, doc) {
                return false;
            }
        }

        // Attributes
        for attr_sel in &self.attributes {
            if !self.matches_attribute(attr_sel, node) {
                return false;
            }
        }

        true
    }

    fn matches_pseudo_class(&self, pc: &PseudoClassSelector, node: &Node, doc: &Document) -> bool {
        match pc {
            PseudoClassSelector::Hover => node.has_pseudo_state(PseudoStateFlags::HOVER),
            PseudoClassSelector::Focus => node.has_pseudo_state(PseudoStateFlags::FOCUS),
            PseudoClassSelector::Active => node.has_pseudo_state(PseudoStateFlags::ACTIVE),
            PseudoClassSelector::Visited => node.has_pseudo_state(PseudoStateFlags::VISITED),
            PseudoClassSelector::Disabled => node.has_pseudo_state(PseudoStateFlags::DISABLED),
            PseudoClassSelector::Checked => node.has_pseudo_state(PseudoStateFlags::CHECKED),
            PseudoClassSelector::FirstChild => node.has_pseudo_state(PseudoStateFlags::FIRST_CHILD),
            PseudoClassSelector::LastChild => node.has_pseudo_state(PseudoStateFlags::LAST_CHILD),
            PseudoClassSelector::FocusWithin => node.has_pseudo_state(PseudoStateFlags::FOCUS_WITHIN),
            PseudoClassSelector::FocusVisible => node.has_pseudo_state(PseudoStateFlags::FOCUS_VISIBLE),
            PseudoClassSelector::PlaceholderShown => node.has_pseudo_state(PseudoStateFlags::PLACEHOLDER_SHOWN),
            PseudoClassSelector::ReadOnly => node.has_pseudo_state(PseudoStateFlags::READ_ONLY),
            PseudoClassSelector::ReadWrite => node.has_pseudo_state(PseudoStateFlags::READ_WRITE),
            PseudoClassSelector::Root => node.has_pseudo_state(PseudoStateFlags::ROOT),
            PseudoClassSelector::Empty => node.has_pseudo_state(PseudoStateFlags::EMPTY),
            PseudoClassSelector::NthChild(anb) => {
                // Determine 1-based index among element siblings (skip text nodes)
                if let Some(parent_id) = node.parent {
                    let children = doc.children(parent_id);
                    let mut elem_index = 0i32;
                    for &c in children {
                        if let Some(child) = doc.get(c) {
                            if child.is_element() {
                                elem_index += 1;
                                if c == node.id {
                                    return anb.matches(elem_index);
                                }
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::NthLastChild(anb) => {
                if let Some(parent_id) = node.parent {
                    let children = doc.children(parent_id);
                    let mut elem_index = 0i32;
                    for &c in children.iter().rev() {
                        if let Some(child) = doc.get(c) {
                            if child.is_element() {
                                elem_index += 1;
                                if c == node.id {
                                    return anb.matches(elem_index);
                                }
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::Not(selectors) => {
                // :not(S1, S2, ...) matches if NONE of the selectors match
                !selectors.iter().any(|s| s.matches(doc, node.id))
            }
            PseudoClassSelector::Is(selectors) | PseudoClassSelector::Where(selectors) => {
                selectors.iter().any(|s| s.matches(doc, node.id))
            }
            PseudoClassSelector::Has(selectors) => {
                const MAX_HAS_DEPTH: u32 = 256;
                fn check_descendants(doc: &Document, parent: NodeId, sel: &ComplexSelector, depth: u32) -> bool {
                    if depth >= MAX_HAS_DEPTH {
                        return false;
                    }
                    for &child_id in doc.children(parent) {
                        if sel.matches(doc, child_id) {
                            return true;
                        }
                        if check_descendants(doc, child_id, sel, depth + 1) {
                            return true;
                        }
                    }
                    false
                }
                // :has(S1, S2, ...) matches if ANY of the selectors match a descendant
                selectors.iter().any(|s| check_descendants(doc, node.id, s, 0))
            }
            PseudoClassSelector::NthOfType(anb) => {
                if let Some(parent_id) = node.parent {
                    let my_tag = node.tag_name();
                    let children = doc.children(parent_id);
                    let mut type_index = 0i32;
                    for &c in children {
                        if let Some(child) = doc.get(c) {
                            if child.tag_name() == my_tag {
                                type_index += 1;
                                if c == node.id {
                                    return anb.matches(type_index);
                                }
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::NthLastOfType(anb) => {
                if let Some(parent_id) = node.parent {
                    let my_tag = node.tag_name();
                    let children = doc.children(parent_id);
                    let mut type_index = 0i32;
                    for &c in children.iter().rev() {
                        if let Some(child) = doc.get(c) {
                            if child.tag_name() == my_tag {
                                type_index += 1;
                                if c == node.id {
                                    return anb.matches(type_index);
                                }
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::LastOfType => {
                if let Some(parent_id) = node.parent {
                    let my_tag = node.tag_name();
                    let children = doc.children(parent_id);
                    for &c in children.iter().rev() {
                        if let Some(child) = doc.get(c) {
                            if child.tag_name() == my_tag {
                                return c == node.id;
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::OnlyChild => {
                if let Some(parent_id) = node.parent {
                    let children = doc.children(parent_id);
                    let mut element_count = 0;
                    let mut only_element_is_self = false;
                    for &c in children {
                        if doc.get(c).map_or(false, |n| n.is_element()) {
                            element_count += 1;
                            if c == node.id {
                                only_element_is_self = true;
                            }
                            if element_count > 1 {
                                return false;
                            }
                        }
                    }
                    return element_count == 1 && only_element_is_self;
                }
                false
            }
            PseudoClassSelector::OnlyOfType => {
                if let Some(parent_id) = node.parent {
                    let my_tag = node.tag_name();
                    let children = doc.children(parent_id);
                    let same_type_count = children.iter().filter(|&&c| {
                        doc.get(c).map_or(false, |child| child.tag_name() == my_tag)
                    }).count();
                    return same_type_count == 1;
                }
                false
            }
            PseudoClassSelector::Target => node.has_pseudo_state(PseudoStateFlags::TARGET),
            PseudoClassSelector::Scope => node.has_pseudo_state(PseudoStateFlags::SCOPE),
            PseudoClassSelector::Lang(lang_code) => {
                // Check lang attribute on this element or ancestors
                let node_lang = node.attrs.get("lang");
                if let Some(node_lang) = node_lang {
                    // Match if lang starts with the code (e.g., "en" matches "en-US")
                    return node_lang == lang_code.as_str()
                        || node_lang.starts_with(&format!("{}-", lang_code));
                }
                // Check ancestors for inherited lang
                let mut current = node.parent;
                while let Some(pid) = current {
                    if let Some(parent) = doc.get(pid) {
                        if let Some(parent_lang) = parent.attrs.get("lang") {
                            return parent_lang == lang_code.as_str()
                                || parent_lang.starts_with(&format!("{}-", lang_code));
                        }
                        current = parent.parent;
                    } else {
                        break;
                    }
                }
                false
            }
            PseudoClassSelector::FirstOfType => {
                if let Some(parent_id) = node.parent {
                    let my_tag = node.tag_name();
                    let children = doc.children(parent_id);
                    for &c in children {
                        if let Some(child) = doc.get(c) {
                            if child.tag_name() == my_tag {
                                return c == node.id;
                            }
                        }
                    }
                }
                false
            }
            PseudoClassSelector::Enabled => !node.has_pseudo_state(PseudoStateFlags::DISABLED),
            PseudoClassSelector::Default => node.attrs.get("default").is_some(),
            PseudoClassSelector::Indeterminate => {
                node.attrs.get("indeterminate").map_or(false, |v| v == "true")
            }
            PseudoClassSelector::Required => node.attrs.get("required").is_some(),
            PseudoClassSelector::Optional => node.attrs.get("required").is_none(),
            PseudoClassSelector::Valid => node.attrs.get("aria-invalid").map_or(true, |v| v != "true"),
            PseudoClassSelector::Invalid => node.attrs.get("aria-invalid").map_or(false, |v| v == "true"),
            PseudoClassSelector::InRange => {
                // Check if value is within min/max bounds
                let value = node.attrs.get("value").and_then(|v| v.parse::<f64>().ok());
                let min = node.attrs.get("min").and_then(|v| v.parse::<f64>().ok());
                let max = node.attrs.get("max").and_then(|v| v.parse::<f64>().ok());
                if let Some(val) = value {
                    let above_min = min.map_or(true, |m| val >= m);
                    let below_max = max.map_or(true, |m| val <= m);
                    return above_min && below_max;
                }
                false
            }
            PseudoClassSelector::OutOfRange => {
                let value = node.attrs.get("value").and_then(|v| v.parse::<f64>().ok());
                let min = node.attrs.get("min").and_then(|v| v.parse::<f64>().ok());
                let max = node.attrs.get("max").and_then(|v| v.parse::<f64>().ok());
                if let Some(val) = value {
                    let below_min = min.map_or(false, |m| val < m);
                    let above_max = max.map_or(false, |m| val > m);
                    return below_min || above_max;
                }
                false
            }
            PseudoClassSelector::Link => {
                // :link matches unvisited links — treat as any link that's not visited
                let is_link_tag = node.tag_name() == "a" || node.tag_name() == "area";
                is_link_tag && node.attrs.contains("href") && !node.has_pseudo_state(PseudoStateFlags::VISITED)
            }
            PseudoClassSelector::AnyLink => {
                let is_link_tag = node.tag_name() == "a" || node.tag_name() == "area";
                is_link_tag && node.attrs.contains("href")
            }
            PseudoClassSelector::Dir(dir) => {
                // Check direction attribute or inherited direction
                if let Some(d) = node.attrs.get("dir") {
                    d.eq_ignore_ascii_case(dir)
                } else {
                    // Default LTR
                    dir.eq_ignore_ascii_case("ltr")
                }
            }
            PseudoClassSelector::Autofill => {
                node.has_pseudo_state(PseudoStateFlags::AUTOFILL)
            }
            PseudoClassSelector::Modal => {
                node.has_pseudo_state(PseudoStateFlags::MODAL)
            }
            PseudoClassSelector::Fullscreen => {
                node.has_pseudo_state(PseudoStateFlags::FULLSCREEN)
            }
        }
    }

    fn matches_attribute(&self, sel: &AttributeSelector, node: &Node) -> bool {
        let value = node.attrs.get(&sel.name);
        
        // Helper for case-insensitive comparison
        let cmp_str = |a: &str, b: &str| -> bool {
            if sel.case_insensitive {
                a.eq_ignore_ascii_case(b)
            } else {
                a == b
            }
        };
        let starts_with_ci = |a: &str, b: &str| -> bool {
            if sel.case_insensitive {
                a.to_ascii_lowercase().starts_with(&b.to_ascii_lowercase())
            } else {
                a.starts_with(b)
            }
        };
        let ends_with_ci = |a: &str, b: &str| -> bool {
            if sel.case_insensitive {
                a.to_ascii_lowercase().ends_with(&b.to_ascii_lowercase())
            } else {
                a.ends_with(b)
            }
        };
        let contains_ci = |a: &str, b: &str| -> bool {
            if sel.case_insensitive {
                a.to_ascii_lowercase().contains(&b.to_ascii_lowercase())
            } else {
                a.contains(b)
            }
        };
        
        match &sel.op {
            AttributeOp::Exists => value.is_some(),
            AttributeOp::Equals(v) => value.map_or(false, |a| cmp_str(a, v.as_str())),
            AttributeOp::Contains(v) => value.map_or(false, |a| {
                a.split_whitespace().any(|w| cmp_str(w, v.as_str()))
            }),
            AttributeOp::DashMatch(v) => value.map_or(false, |a| {
                cmp_str(a, v.as_str()) || starts_with_ci(a, &format!("{}-", v))
            }),
            AttributeOp::Prefix(v) => value.map_or(false, |a| starts_with_ci(a, v.as_str())),
            AttributeOp::Suffix(v) => value.map_or(false, |a| ends_with_ci(a, v.as_str())),
            AttributeOp::Substring(v) => value.map_or(false, |a| contains_ci(a, v.as_str())),
        }
    }
}

impl Default for CompoundSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// A complex selector: a chain of compound selectors joined by combinators.
///
/// `compounds[0]` is matched first (rightmost in CSS), then we walk left.
/// `compounds[i]` must match via `combinators[i-1]` relative to `compounds[i-1]`'s match.
///
/// For a simple selector like `div.foo`, there is one compound and no combinators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelector {
    /// Compound selectors from right to left.
    pub compounds: Vec<CompoundSelector>,
    /// Combinators between compounds. `len() == compounds.len() - 1`.
    pub combinators: Vec<Combinator>,
}

impl ComplexSelector {
    /// A simple selector (single compound, no combinators).
    pub fn simple(compound: CompoundSelector) -> Self {
        Self {
            compounds: vec![compound],
            combinators: Vec::new(),
        }
    }

    /// Calculate overall specificity.
    pub fn specificity(&self) -> Specificity {
        let mut total = Specificity::ZERO;
        for c in &self.compounds {
            total = total.add(c.specificity());
        }
        total
    }

    /// Check if this complex selector matches a node in the document.
    pub fn matches(&self, doc: &Document, node_id: NodeId) -> bool {
        if self.compounds.is_empty() {
            return false;
        }

        let node = match doc.get(node_id) {
            Some(n) => n,
            None => return false,
        };

        // First compound must match the target node
        if !self.compounds[0].matches_node(node, doc) {
            return false;
        }

        // Recursively match remaining combinators with backtracking
        Self::match_rest(&self.compounds, &self.combinators, 0, node_id, doc)
    }

    /// Recursively match combinators starting at index `idx`, with `current` as
    /// the node matched by `compounds[idx]`. Returns true if all remaining
    /// combinators can be satisfied.
    fn match_rest(
        compounds: &[CompoundSelector],
        combinators: &[Combinator],
        idx: usize,
        current: NodeId,
        doc: &Document,
    ) -> bool {
        if idx >= combinators.len() {
            return true; // all combinators matched
        }

        let combinator = combinators[idx];
        let next_compound = &compounds[idx + 1];

        match combinator {
            Combinator::Child => {
                let parent_id = match doc.parent(current) {
                    Some(p) => p,
                    None => return false,
                };
                let parent = match doc.get(parent_id) {
                    Some(n) => n,
                    None => return false,
                };
                if !next_compound.matches_node(parent, doc) {
                    return false;
                }
                Self::match_rest(compounds, combinators, idx + 1, parent_id, doc)
            }
            Combinator::Descendant => {
                // Try each ancestor; backtrack if subsequent combinators fail
                let mut anc = doc.parent(current);
                while let Some(anc_id) = anc {
                    if let Some(anc_node) = doc.get(anc_id) {
                        if next_compound.matches_node(anc_node, doc)
                            && Self::match_rest(compounds, combinators, idx + 1, anc_id, doc)
                        {
                            return true;
                        }
                        anc = anc_node.parent;
                    } else {
                        break;
                    }
                }
                false
            }
            Combinator::NextSibling => {
                let parent_id = match doc.parent(current) {
                    Some(p) => p,
                    None => return false,
                };
                let children = doc.children(parent_id);
                let pos = match children.iter().position(|&c| c == current) {
                    Some(p) => p,
                    None => return false,
                };
                // Find the previous element sibling
                let mut prev_elem = None;
                for &sib_id in children[..pos].iter().rev() {
                    if doc.get(sib_id).map_or(false, |n| n.is_element()) {
                        prev_elem = Some(sib_id);
                        break;
                    }
                }
                let prev_id = match prev_elem {
                    Some(id) => id,
                    None => return false,
                };
                let prev = match doc.get(prev_id) {
                    Some(n) => n,
                    None => return false,
                };
                if !next_compound.matches_node(prev, doc) {
                    return false;
                }
                Self::match_rest(compounds, combinators, idx + 1, prev_id, doc)
            }
            Combinator::SubsequentSibling => {
                // Try each preceding element sibling; backtrack if needed
                let parent_id = match doc.parent(current) {
                    Some(p) => p,
                    None => return false,
                };
                let children = doc.children(parent_id);
                let pos = match children.iter().position(|&c| c == current) {
                    Some(p) => p,
                    None => return false,
                };
                for &sib_id in children[..pos].iter().rev() {
                    if let Some(sib) = doc.get(sib_id) {
                        if sib.is_element()
                            && next_compound.matches_node(sib, doc)
                            && Self::match_rest(compounds, combinators, idx + 1, sib_id, doc)
                        {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    /// Parse a CSS selector string into a ComplexSelector.
    ///
    /// Supports: tag, .class, #id, :pseudo, [attr], combinators (` `, `>`, `+`, `~`).
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        // Tokenize by splitting on combinator characters while preserving them
        let mut compounds = Vec::new();
        let mut combinators = Vec::new();
        let mut current_segment = String::new();

        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            match chars[i] {
                '>' | '+' | '~' => {
                    let seg = current_segment.trim().to_string();
                    if !seg.is_empty() {
                        compounds.push(parse_compound(&seg)?);
                    }
                    let comb = match chars[i] {
                        '>' => Combinator::Child,
                        '+' => Combinator::NextSibling,
                        '~' => Combinator::SubsequentSibling,
                        _ => unreachable!(),
                    };
                    combinators.push(comb);
                    current_segment.clear();
                    i += 1;
                }
                ' ' => {
                    // Could be descendant combinator or just whitespace around > + ~
                    let seg = current_segment.trim().to_string();
                    // Skip whitespace
                    while i < len && chars[i] == ' ' {
                        i += 1;
                    }
                    // Check if next char is an explicit combinator
                    if i < len && matches!(chars[i], '>' | '+' | '~') {
                        // This space is just padding around an explicit combinator
                        if !seg.is_empty() {
                            compounds.push(parse_compound(&seg)?);
                        }
                        current_segment.clear();
                        continue; // will be handled in next iteration
                    }
                    // This space IS the descendant combinator
                    if !seg.is_empty() {
                        compounds.push(parse_compound(&seg)?);
                        combinators.push(Combinator::Descendant);
                    }
                    current_segment.clear();
                    continue; // don't increment i — already advanced past spaces
                }
                c => {
                    current_segment.push(c);
                    i += 1;
                }
            }
        }

        // Last segment
        let seg = current_segment.trim().to_string();
        if !seg.is_empty() {
            compounds.push(parse_compound(&seg)?);
        }

        if compounds.is_empty() {
            return None;
        }

        // CSS selectors are matched right-to-left, so reverse for our storage
        compounds.reverse();
        combinators.reverse();

        Some(ComplexSelector { compounds, combinators })
    }
}

/// Parse a single compound selector string like `div.foo#bar:hover[type="text"]`.
fn parse_compound(input: &str) -> Option<CompoundSelector> {
    let mut sel = CompoundSelector::new();
    let mut chars = input.chars().peekable();
    let mut current = String::new();
    let mut mode = 'T'; // T=tag, .=class, #=id, :=pseudo, [=attr

    while let Some(&ch) = chars.peek() {
        match ch {
            '.' | '#' | ':' | '[' => {
                flush_segment(&mut sel, mode, &current);
                current.clear();
                mode = ch;
                chars.next();
            }
            ']' => {
                chars.next();
                // Parse attribute selector
                parse_attribute_into(&mut sel, &current);
                current.clear();
                mode = 'T'; // Reset
            }
            _ => {
                current.push(ch);
                chars.next();
            }
        }
    }
    flush_segment(&mut sel, mode, &current);

    Some(sel)
}

fn flush_segment(sel: &mut CompoundSelector, mode: char, value: &str) {
    if value.is_empty() {
        return;
    }
    match mode {
        'T' => {
            if value != "*" {
                sel.tag = Some(value.to_string());
            }
        }
        '.' => sel.classes.push(value.to_string()),
        '#' => sel.id = Some(value.to_string()),
        ':' => {
            if let Some(stripped) = value.strip_prefix(':') {
                // Pseudo-element (::before, ::after, etc.)
                sel.pseudo_element = parse_pseudo_element(stripped);
            } else if let Some(pc) = parse_pseudo_class(value) {
                sel.pseudo_classes.push(pc);
            }
        }
        '[' => {
            parse_attribute_into(sel, value);
        }
        _ => {}
    }
}

fn parse_pseudo_element(name: &str) -> Option<PseudoElement> {
    match name {
        "before" => Some(PseudoElement::Before),
        "after" => Some(PseudoElement::After),
        "first-line" => Some(PseudoElement::FirstLine),
        "first-letter" => Some(PseudoElement::FirstLetter),
        "placeholder" => Some(PseudoElement::Placeholder),
        "selection" => Some(PseudoElement::Selection),
        _ => None,
    }
}

/// Maximum number of selectors in a `:is()`, `:not()`, `:where()`, or `:has()` list.
const MAX_SELECTOR_LIST: usize = 1024;

fn parse_pseudo_class(name: &str) -> Option<PseudoClassSelector> {
    match name {
        "hover" => Some(PseudoClassSelector::Hover),
        "focus" => Some(PseudoClassSelector::Focus),
        "active" => Some(PseudoClassSelector::Active),
        "visited" => Some(PseudoClassSelector::Visited),
        "disabled" => Some(PseudoClassSelector::Disabled),
        "checked" => Some(PseudoClassSelector::Checked),
        "first-child" => Some(PseudoClassSelector::FirstChild),
        "last-child" => Some(PseudoClassSelector::LastChild),
        "focus-within" => Some(PseudoClassSelector::FocusWithin),
        "focus-visible" => Some(PseudoClassSelector::FocusVisible),
        "placeholder-shown" => Some(PseudoClassSelector::PlaceholderShown),
        "read-only" => Some(PseudoClassSelector::ReadOnly),
        "read-write" => Some(PseudoClassSelector::ReadWrite),
        "root" => Some(PseudoClassSelector::Root),
        "empty" => Some(PseudoClassSelector::Empty),
        "last-of-type" => Some(PseudoClassSelector::LastOfType),
        "only-child" => Some(PseudoClassSelector::OnlyChild),
        "only-of-type" => Some(PseudoClassSelector::OnlyOfType),
        "target" => Some(PseudoClassSelector::Target),
        "scope" => Some(PseudoClassSelector::Scope),
        "first-of-type" => Some(PseudoClassSelector::FirstOfType),
        "enabled" => Some(PseudoClassSelector::Enabled),
        "default" => Some(PseudoClassSelector::Default),
        "indeterminate" => Some(PseudoClassSelector::Indeterminate),
        "required" => Some(PseudoClassSelector::Required),
        "optional" => Some(PseudoClassSelector::Optional),
        "valid" => Some(PseudoClassSelector::Valid),
        "invalid" => Some(PseudoClassSelector::Invalid),
        "in-range" => Some(PseudoClassSelector::InRange),
        "out-of-range" => Some(PseudoClassSelector::OutOfRange),
        "link" => Some(PseudoClassSelector::Link),
        "any-link" => Some(PseudoClassSelector::AnyLink),
        "autofill" => Some(PseudoClassSelector::Autofill),
        "modal" => Some(PseudoClassSelector::Modal),
        "fullscreen" => Some(PseudoClassSelector::Fullscreen),
        _ if name.starts_with("nth-child(") && name.ends_with(')') => {
            let expr = &name[10..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthChild)
        }
        _ if name.starts_with("nth-last-child(") && name.ends_with(')') => {
            let expr = &name[15..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthLastChild)
        }
        _ if name.starts_with("is(") && name.ends_with(')') => {
            let inner = &name[3..name.len() - 1];
            let selectors: Vec<ComplexSelector> = inner.split(',')
                .filter_map(|s| ComplexSelector::parse(s.trim()))
                .take(MAX_SELECTOR_LIST)
                .collect();
            if selectors.is_empty() { None } else { Some(PseudoClassSelector::Is(selectors)) }
        }
        _ if name.starts_with("where(") && name.ends_with(')') => {
            let inner = &name[6..name.len() - 1];
            let selectors: Vec<ComplexSelector> = inner.split(',')
                .filter_map(|s| ComplexSelector::parse(s.trim()))
                .take(MAX_SELECTOR_LIST)
                .collect();
            if selectors.is_empty() { None } else { Some(PseudoClassSelector::Where(selectors)) }
        }
        _ if name.starts_with("has(") && name.ends_with(')') => {
            let inner = &name[4..name.len() - 1];
            let selectors: Vec<ComplexSelector> = inner.split(',')
                .filter_map(|s| ComplexSelector::parse(s.trim()))
                .take(MAX_SELECTOR_LIST)
                .collect();
            if selectors.is_empty() { None } else { Some(PseudoClassSelector::Has(selectors)) }
        }
        _ if name.starts_with("not(") && name.ends_with(')') => {
            let inner = &name[4..name.len() - 1];
            // :not() takes a selector list per Selectors Level 4
            let selectors: Vec<ComplexSelector> = inner.split(',')
                .filter_map(|s| ComplexSelector::parse(s.trim()))
                .take(MAX_SELECTOR_LIST)
                .collect();
            if selectors.is_empty() {
                None
            } else {
                Some(PseudoClassSelector::Not(selectors))
            }
        }
        _ if name.starts_with("nth-of-type(") && name.ends_with(')') => {
            let expr = &name[12..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthOfType)
        }
        _ if name.starts_with("nth-last-of-type(") && name.ends_with(')') => {
            let expr = &name[17..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthLastOfType)
        }
        _ if name.starts_with("lang(") && name.ends_with(')') => {
            let lang_code = &name[5..name.len() - 1];
            let lang_code = lang_code.trim().trim_matches('"').trim_matches('\'');
            if lang_code.is_empty() {
                None
            } else {
                Some(PseudoClassSelector::Lang(lang_code.to_string()))
            }
        }
        _ if name.starts_with("dir(") && name.ends_with(')') => {
            let dir_val = &name[4..name.len() - 1];
            Some(PseudoClassSelector::Dir(dir_val.trim().to_string()))
        }
        _ => None,
    }
}

fn parse_anb(expr: &str) -> Option<AnB> {
    let expr = expr.trim();
    match expr {
        "odd" => return Some(AnB { a: 2, b: 1 }),
        "even" => return Some(AnB { a: 2, b: 0 }),
        _ => {}
    }
    if let Ok(n) = expr.parse::<i32>() {
        return Some(AnB { a: 0, b: n });
    }
    // Parse an+b
    if let Some(pos) = expr.find('n') {
        let a_str = &expr[..pos].trim();
        let a = if a_str.is_empty() || *a_str == "+" {
            1
        } else if *a_str == "-" {
            -1
        } else {
            a_str.parse::<i32>().ok()?
        };
        let rest = &expr[pos + 1..].trim();
        let b = if rest.is_empty() {
            0
        } else {
            rest.replace(' ', "").parse::<i32>().ok()?
        };
        Some(AnB { a, b })
    } else {
        None
    }
}

fn parse_attribute_into(sel: &mut CompoundSelector, input: &str) {
    let input = input.trim();
    
    // Check for case-insensitivity flag at the end: [attr=value i] or [attr=value s]
    let (input, case_insensitive) = if input.ends_with(" i") || input.ends_with(" I") {
        (&input[..input.len() - 2], true)
    } else if input.ends_with(" s") || input.ends_with(" S") {
        // 's' flag means case-sensitive (the default)
        (&input[..input.len() - 2], false)
    } else {
        (input, false)
    };
    
    // Try various operators
    for (op_str, make_op) in &[
        ("~=", AttributeOp::Contains as fn(String) -> AttributeOp),
        ("|=", AttributeOp::DashMatch as fn(String) -> AttributeOp),
        ("^=", AttributeOp::Prefix as fn(String) -> AttributeOp),
        ("$=", AttributeOp::Suffix as fn(String) -> AttributeOp),
        ("*=", AttributeOp::Substring as fn(String) -> AttributeOp),
        ("=", AttributeOp::Equals as fn(String) -> AttributeOp),
    ] {
        if let Some(pos) = input.find(op_str) {
            let name = input[..pos].trim().to_string();
            let val = input[pos + op_str.len()..].trim();
            let val = val.trim_matches('"').trim_matches('\'').to_string();
            sel.attributes.push(AttributeSelector {
                name,
                op: make_op(val),
                case_insensitive,
            });
            return;
        }
    }
    // No operator — just [attr]
    sel.attributes.push(AttributeSelector {
        name: input.to_string(),
        op: AttributeOp::Exists,
        case_insensitive: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tag() {
        let sel = ComplexSelector::parse("div").unwrap();
        assert_eq!(sel.compounds.len(), 1);
        assert_eq!(sel.compounds[0].tag, Some("div".into()));
    }

    #[test]
    fn parse_class() {
        let sel = ComplexSelector::parse(".active").unwrap();
        assert_eq!(sel.compounds[0].classes, vec!["active".to_string()]);
    }

    #[test]
    fn parse_id() {
        let sel = ComplexSelector::parse("#main").unwrap();
        assert_eq!(sel.compounds[0].id, Some("main".into()));
    }

    #[test]
    fn parse_compound() {
        let sel = ComplexSelector::parse("div.foo.bar#baz:hover").unwrap();
        let c = &sel.compounds[0];
        assert_eq!(c.tag, Some("div".into()));
        assert!(c.classes.contains(&"foo".to_string()));
        assert!(c.classes.contains(&"bar".to_string()));
        assert_eq!(c.id, Some("baz".into()));
        assert_eq!(c.pseudo_classes, vec![PseudoClassSelector::Hover]);
    }

    #[test]
    fn parse_descendant() {
        let sel = ComplexSelector::parse("div span").unwrap();
        assert_eq!(sel.compounds.len(), 2);
        // Stored right-to-left: compounds[0] = span, compounds[1] = div
        assert_eq!(sel.compounds[0].tag, Some("span".into()));
        assert_eq!(sel.compounds[1].tag, Some("div".into()));
        assert_eq!(sel.combinators, vec![Combinator::Descendant]);
    }

    #[test]
    fn parse_child_combinator() {
        let sel = ComplexSelector::parse("ul > li").unwrap();
        assert_eq!(sel.compounds.len(), 2);
        assert_eq!(sel.compounds[0].tag, Some("li".into()));
        assert_eq!(sel.compounds[1].tag, Some("ul".into()));
        assert_eq!(sel.combinators, vec![Combinator::Child]);
    }

    #[test]
    fn parse_adjacent_sibling() {
        let sel = ComplexSelector::parse("h1 + p").unwrap();
        assert_eq!(sel.combinators, vec![Combinator::NextSibling]);
    }

    #[test]
    fn parse_general_sibling() {
        let sel = ComplexSelector::parse("h1 ~ p").unwrap();
        assert_eq!(sel.combinators, vec![Combinator::SubsequentSibling]);
    }

    #[test]
    fn specificity_calc() {
        // div.foo#bar = (1, 1, 1)
        let sel = ComplexSelector::parse("div.foo#bar").unwrap();
        assert_eq!(sel.specificity(), Specificity::new(1, 1, 1));

        // .a.b.c = (0, 3, 0)
        let sel = ComplexSelector::parse(".a.b.c").unwrap();
        assert_eq!(sel.specificity(), Specificity::new(0, 3, 0));
    }

    #[test]
    fn anb_matching() {
        let odd = AnB { a: 2, b: 1 };
        assert!(odd.matches(1));
        assert!(!odd.matches(2));
        assert!(odd.matches(3));

        let even = AnB { a: 2, b: 0 };
        assert!(!even.matches(1));
        assert!(even.matches(2));
        assert!(!even.matches(3));
        assert!(even.matches(4));

        let third = AnB { a: 3, b: 0 };
        assert!(third.matches(3));
        assert!(third.matches(6));
        assert!(!third.matches(4));
    }

    #[test]
    fn matches_dom_simple() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);
        doc.add_class(div, "active");
        doc.set_id(div, "main");

        let sel = ComplexSelector::parse("div.active#main").unwrap();
        assert!(sel.matches(&doc, div));

        let sel2 = ComplexSelector::parse("span.active").unwrap();
        assert!(!sel2.matches(&doc, div));
    }

    #[test]
    fn matches_dom_child_combinator() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("ul");
        let child = doc.create_element("li");
        doc.append_child(root, parent);
        doc.append_child(parent, child);

        let sel = ComplexSelector::parse("ul > li").unwrap();
        assert!(sel.matches(&doc, child));

        // li is NOT a direct child of root
        let sel2 = ComplexSelector::parse("root > li").unwrap();
        assert!(!sel2.matches(&doc, child));
    }

    #[test]
    fn matches_dom_descendant() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("div");
        let b = doc.create_element("span");
        let c = doc.create_element("em");
        doc.append_child(root, a);
        doc.append_child(a, b);
        doc.append_child(b, c);

        let sel = ComplexSelector::parse("div em").unwrap();
        assert!(sel.matches(&doc, c));
    }
}
