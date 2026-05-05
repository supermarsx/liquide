use liquide_dom::Document;
use liquide_style_engine::selector::{AttributeOp, ComplexSelector, PseudoClassSelector};

#[test]
fn relative_has_child_matches_only_direct_children() {
    let mut doc = Document::new();
    let root = doc.root();

    let section_with_child = doc.create_element("section");
    let direct_img = doc.create_element("img");
    doc.append_child(root, section_with_child);
    doc.append_child(section_with_child, direct_img);

    let section_with_nested = doc.create_element("section");
    let wrapper = doc.create_element("div");
    let nested_img = doc.create_element("img");
    doc.append_child(root, section_with_nested);
    doc.append_child(section_with_nested, wrapper);
    doc.append_child(wrapper, nested_img);

    let selector = ComplexSelector::parse("section:has(> img)").unwrap();
    assert!(selector.matches(&doc, section_with_child));
    assert!(!selector.matches(&doc, section_with_nested));
}

#[test]
fn nested_selector_lists_and_quoted_attributes_parse_conservatively() {
    let selector =
        ComplexSelector::parse(r#"button:not(:is(.active, [data-state="open,now"]))"#).unwrap();

    match &selector.compounds[0].pseudo_classes[0] {
        PseudoClassSelector::Not(selectors) => {
            assert_eq!(selectors.len(), 1);
            assert_eq!(selectors[0].compounds[0].pseudo_classes.len(), 1);
        }
        other => panic!("unexpected pseudo-class: {other:?}"),
    }

    let attribute_selector =
        ComplexSelector::parse(r#"a[href^="https://example.com?q=.foo"]"#).unwrap();
    assert_eq!(
        attribute_selector.compounds[0].attributes[0].op,
        AttributeOp::Prefix("https://example.com?q=.foo".to_string())
    );
}

#[test]
fn unsupported_shadow_dom_pseudos_fail_closed() {
    assert!(ComplexSelector::parse(":host").is_none());
    assert!(ComplexSelector::parse("div::slotted(span)").is_none());
}

#[test]
fn lang_and_dir_follow_css_inheritance_rules() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("div");
    let child = doc.create_element("span");
    doc.append_child(root, parent);
    doc.append_child(parent, child);
    doc.set_attribute(parent, "lang", "en-US");
    doc.set_attribute(parent, "dir", "rtl");

    let lang_selector = ComplexSelector::parse(":lang(EN)").unwrap();
    let dir_selector = ComplexSelector::parse(":dir(rtl)").unwrap();

    assert!(lang_selector.matches(&doc, child));
    assert!(dir_selector.matches(&doc, child));
}
