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
    Not(Box<ComplexSelector>),
    FocusWithin,
    FocusVisible,
    PlaceholderShown,
    ReadOnly,
    ReadWrite,
    Root,
    Empty,
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

/// An attribute selector like `[type="submit"]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub op: AttributeOp,
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
}

impl CompoundSelector {
    pub fn new() -> Self {
        Self {
            tag: None,
            id: None,
            classes: Vec::new(),
            pseudo_classes: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Calculate the specificity contribution of this compound.
    pub fn specificity(&self) -> Specificity {
        let id = if self.id.is_some() { 1 } else { 0 };
        let mut class = self.classes.len() as u32
            + self.attributes.len() as u32;
        for pc in &self.pseudo_classes {
            match pc {
                PseudoClassSelector::Not(inner) => {
                    // :not() adds the specificity of its argument
                    let inner_spec = inner.specificity();
                    return Specificity::new(id, class, if self.tag.is_some() { 1 } else { 0 })
                        .add(inner_spec);
                }
                _ => class += 1,
            }
        }
        let type_sel = if self.tag.is_some() { 1 } else { 0 };
        Specificity::new(id, class, type_sel)
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
                // Determine 1-based index among siblings
                if let Some(parent_id) = node.parent {
                    let children = doc.children(parent_id);
                    if let Some(index) = children.iter().position(|&c| c == node.id) {
                        return anb.matches((index + 1) as i32);
                    }
                }
                false
            }
            PseudoClassSelector::NthLastChild(anb) => {
                if let Some(parent_id) = node.parent {
                    let children = doc.children(parent_id);
                    if let Some(index) = children.iter().position(|&c| c == node.id) {
                        let from_end = (children.len() - index) as i32;
                        return anb.matches(from_end);
                    }
                }
                false
            }
            PseudoClassSelector::Not(inner) => {
                // :not(S) matches if S does NOT match
                !inner.matches(doc, node.id)
            }
        }
    }

    fn matches_attribute(&self, sel: &AttributeSelector, node: &Node) -> bool {
        let value = node.attrs.get(&sel.name);
        match &sel.op {
            AttributeOp::Exists => value.is_some(),
            AttributeOp::Equals(v) => value.map_or(false, |a| a == v.as_str()),
            AttributeOp::Contains(v) => value.map_or(false, |a| {
                a.split_whitespace().any(|w| w == v.as_str())
            }),
            AttributeOp::DashMatch(v) => value.map_or(false, |a| {
                a == v.as_str() || a.starts_with(&format!("{}-", v))
            }),
            AttributeOp::Prefix(v) => value.map_or(false, |a| a.starts_with(v.as_str())),
            AttributeOp::Suffix(v) => value.map_or(false, |a| a.ends_with(v.as_str())),
            AttributeOp::Substring(v) => value.map_or(false, |a| a.contains(v.as_str())),
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

        // Walk left through combinators
        let mut current = node_id;
        for i in 0..self.combinators.len() {
            let combinator = self.combinators[i];
            let next_compound = &self.compounds[i + 1];

            match combinator {
                Combinator::Child => {
                    // Parent must match
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
                    current = parent_id;
                }
                Combinator::Descendant => {
                    // Some ancestor must match
                    let ancestors = doc.ancestors(current);
                    let mut found = false;
                    for &anc_id in &ancestors {
                        if let Some(anc) = doc.get(anc_id) {
                            if next_compound.matches_node(anc, doc) {
                                current = anc_id;
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return false;
                    }
                }
                Combinator::NextSibling => {
                    // Previous sibling must match
                    let parent_id = match doc.parent(current) {
                        Some(p) => p,
                        None => return false,
                    };
                    let children = doc.children(parent_id);
                    let pos = match children.iter().position(|&c| c == current) {
                        Some(p) if p > 0 => p,
                        _ => return false,
                    };
                    let prev_id = children[pos - 1];
                    let prev = match doc.get(prev_id) {
                        Some(n) => n,
                        None => return false,
                    };
                    if !next_compound.matches_node(prev, doc) {
                        return false;
                    }
                    current = prev_id;
                }
                Combinator::SubsequentSibling => {
                    // Some preceding sibling must match
                    let parent_id = match doc.parent(current) {
                        Some(p) => p,
                        None => return false,
                    };
                    let children = doc.children(parent_id);
                    let pos = match children.iter().position(|&c| c == current) {
                        Some(p) => p,
                        None => return false,
                    };
                    let mut found = false;
                    for &sib_id in &children[..pos] {
                        if let Some(sib) = doc.get(sib_id) {
                            if next_compound.matches_node(sib, doc) {
                                current = sib_id;
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return false;
                    }
                }
            }
        }

        true
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
            if let Some(pc) = parse_pseudo_class(value) {
                sel.pseudo_classes.push(pc);
            }
        }
        '[' => {
            parse_attribute_into(sel, value);
        }
        _ => {}
    }
}

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
        _ if name.starts_with("nth-child(") && name.ends_with(')') => {
            let expr = &name[10..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthChild)
        }
        _ if name.starts_with("nth-last-child(") && name.ends_with(')') => {
            let expr = &name[15..name.len() - 1];
            parse_anb(expr).map(PseudoClassSelector::NthLastChild)
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
            });
            return;
        }
    }
    // No operator — just [attr]
    sel.attributes.push(AttributeSelector {
        name: input.to_string(),
        op: AttributeOp::Exists,
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
