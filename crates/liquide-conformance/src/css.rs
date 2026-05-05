#![allow(dead_code)]

//! Shared CSS conformance fixture catalog.
//!
//! This module keeps the post-t13 CSS regression surface in one place so
//! crate-local tests can import the same fixture list without duplicating the
//! coverage matrix in ad hoc assertions.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CssCoverageArea {
    ParserValues,
    Selectors,
    Shorthands,
    SupportsMedia,
    ImportResolution,
    ScopeBounds,
    CustomProperties,
    ThemeRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CssCaseKind {
    Positive,
    Negative,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CssCaseMeta {
    pub id: &'static str,
    pub title: &'static str,
    pub branch: &'static str,
    pub area: CssCoverageArea,
    pub kind: CssCaseKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeCssParserScenario {
    CustomPropertyRoundTrip,
    NestedMathPreservesStructure,
    EmptyMathRejected,
    ShorthandTokensPreserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeCssParserFixture {
    pub meta: CssCaseMeta,
    pub source: &'static str,
    pub scenario: ThemeCssParserScenario,
}

pub const THEME_CSS_PARSER_FIXTURES: &[ThemeCssParserFixture] = &[
    ThemeCssParserFixture {
        meta: CssCaseMeta {
            id: "CSS-PAR-001",
            title: "custom property tokens round-trip as CSS",
            branch: "t13-e2",
            area: CssCoverageArea::ParserValues,
            kind: CssCaseKind::Recovery,
        },
        source: r#"env(titlebar-area-x, 0px) url("foo bar.svg") rgb(255 0 0 / var(--alpha))"#,
        scenario: ThemeCssParserScenario::CustomPropertyRoundTrip,
    },
    ThemeCssParserFixture {
        meta: CssCaseMeta {
            id: "CSS-PAR-002",
            title: "unitless numbers and nested math preserve structure",
            branch: "t13-e2",
            area: CssCoverageArea::ParserValues,
            kind: CssCaseKind::Positive,
        },
        source: "clamp(1, 50% + 2px, 20rem)",
        scenario: ThemeCssParserScenario::NestedMathPreservesStructure,
    },
    ThemeCssParserFixture {
        meta: CssCaseMeta {
            id: "CSS-PAR-003",
            title: "empty math functions are rejected fail-closed",
            branch: "t13-e2",
            area: CssCoverageArea::ParserValues,
            kind: CssCaseKind::Negative,
        },
        source: "min()",
        scenario: ThemeCssParserScenario::EmptyMathRejected,
    },
    ThemeCssParserFixture {
        meta: CssCaseMeta {
            id: "CSS-SH-001",
            title: "layered backgrounds and raw shorthands stay intact",
            branch: "t13-e3",
            area: CssCoverageArea::Shorthands,
            kind: CssCaseKind::Recovery,
        },
        source: r#"
            button {
                background: url(bg.png) center/cover no-repeat, linear-gradient(red, blue);
                font: italic 700 16px/1.4 "Fira Sans", sans-serif;
                animation: fade 1s steps(4, end);
            }
        "#,
        scenario: ThemeCssParserScenario::ShorthandTokensPreserved,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeCssStylesheetScenario {
    InvalidSupportsAndMediaFailClosed,
    ImportQualifiersRespected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeCssStylesheetFixture {
    pub meta: CssCaseMeta,
    pub source: &'static str,
    pub scenario: ThemeCssStylesheetScenario,
}

pub const THEME_CSS_STYLESHEET_FIXTURES: &[ThemeCssStylesheetFixture] = &[
    ThemeCssStylesheetFixture {
        meta: CssCaseMeta {
            id: "CSS-SUP-001",
            title: "invalid supports and media queries fail closed",
            branch: "t13-e4",
            area: CssCoverageArea::SupportsMedia,
            kind: CssCaseKind::Recovery,
        },
        source: r#"
            button { color: #ff0000; }
            @supports (display: definitely-not-a-real-value) {
                button { color: #0000ff; }
            }
            @media (totally-unknown: 1) {
                button { background-color: #0000ff; }
            }
        "#,
        scenario: ThemeCssStylesheetScenario::InvalidSupportsAndMediaFailClosed,
    },
    ThemeCssStylesheetFixture {
        meta: CssCaseMeta {
            id: "CSS-IMP-001",
            title: "import qualifiers gate imported rules",
            branch: "t13-e4",
            area: CssCoverageArea::ImportResolution,
            kind: CssCaseKind::Recovery,
        },
        source: "button { background-color: #0000ff; }",
        scenario: ThemeCssStylesheetScenario::ImportQualifiersRespected,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleEngineScenario {
    RelativeHasChildSelector,
    UnsupportedPseudoFailsClosed,
    QuotedAttributeSelector,
    LangAndDirInheritance,
    SupportsAndMediaFailClosed,
    ScopeEndBounds,
    IncrementalCustomPropertyScope,
    ShadowBoundaryIsolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleEngineFixture {
    pub meta: CssCaseMeta,
    pub css: &'static str,
    pub scenario: StyleEngineScenario,
}

pub const STYLE_ENGINE_FIXTURES: &[StyleEngineFixture] = &[
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SEL-001",
            title: "relative :has child selectors only match direct children",
            branch: "t13-e1",
            area: CssCoverageArea::Selectors,
            kind: CssCaseKind::Positive,
        },
        css: r#"
            section { color: #ff0000; }
            section:has(> img) { color: #00ff00; }
        "#,
        scenario: StyleEngineScenario::RelativeHasChildSelector,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SEL-002",
            title: "unsupported shadow-DOM pseudos fail closed",
            branch: "t13-e1",
            area: CssCoverageArea::Selectors,
            kind: CssCaseKind::Negative,
        },
        css: r#"
            div { color: #ff0000; }
            div::slotted(span) { color: #0000ff; }
        "#,
        scenario: StyleEngineScenario::UnsupportedPseudoFailsClosed,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SEL-003",
            title: "quoted attribute selectors keep their token boundaries",
            branch: "t13-e1",
            area: CssCoverageArea::Selectors,
            kind: CssCaseKind::Recovery,
        },
        css: r#"
            a { color: #ff0000; }
            a[href^="https://example.com?q=.foo"] { color: #00ff00; }
        "#,
        scenario: StyleEngineScenario::QuotedAttributeSelector,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SEL-004",
            title: ":lang and :dir follow inherited state",
            branch: "t13-e1",
            area: CssCoverageArea::Selectors,
            kind: CssCaseKind::Recovery,
        },
        css: r#"
            :lang(en) { color: #00ff00; }
            :dir(rtl) { background-color: #0000ff; }
        "#,
        scenario: StyleEngineScenario::LangAndDirInheritance,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SUP-002",
            title: "runtime supports/media evaluation fails closed",
            branch: "t13-e4",
            area: CssCoverageArea::SupportsMedia,
            kind: CssCaseKind::Recovery,
        },
        css: "",
        scenario: StyleEngineScenario::SupportsAndMediaFailClosed,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-SCP-001",
            title: "@scope end bounds stop matches past the boundary",
            branch: "t13-e4",
            area: CssCoverageArea::ScopeBounds,
            kind: CssCaseKind::Recovery,
        },
        css: r#"
            @scope (.panel) to (.limit) {
                button { color: #00ff00; }
            }
            button { color: #ff0000; }
        "#,
        scenario: StyleEngineScenario::ScopeEndBounds,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-VAR-001",
            title: "incremental invalidation preserves inherited custom property scope",
            branch: "t13-e5",
            area: CssCoverageArea::CustomProperties,
            kind: CssCaseKind::Recovery,
        },
        css: r#"
            .red-scope { --accent: #ff0000; }
            .blue-scope { --accent: #0000ff; }
            .target { color: var(--accent); }
        "#,
        scenario: StyleEngineScenario::IncrementalCustomPropertyScope,
    },
    StyleEngineFixture {
        meta: CssCaseMeta {
            id: "CSS-VAR-002",
            title: "shadow-root boundaries block variable leakage",
            branch: "t13-e5",
            area: CssCoverageArea::CustomProperties,
            kind: CssCaseKind::Negative,
        },
        css: r#"
            .host { --accent: #ff0000; }
            .other { --accent: #0000ff; }
            .inner { color: var(--accent, #00ff00); }
        "#,
        scenario: StyleEngineScenario::ShadowBoundaryIsolation,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalCssCoverage {
    pub meta: CssCaseMeta,
    pub validating_suite: &'static str,
    pub note: &'static str,
}

pub const EXTERNAL_CSS_COVERAGE: &[ExternalCssCoverage] = &[ExternalCssCoverage {
    meta: CssCaseMeta {
        id: "CSS-THEME-001",
        title: "theme activation and inheritance stay covered by theme-engine tests",
        branch: "t13-e6",
        area: CssCoverageArea::ThemeRuntime,
        kind: CssCaseKind::Recovery,
    },
    validating_suite: "cargo test -p liquide-theme-engine --lib",
    note: "Theme activation, inheritance resolution, and emitted-token coverage live in the theme-engine crate surface, which is outside this executor's writable test scope.",
}];

pub const REQUIRED_RUNNABLE_AREAS: &[CssCoverageArea] = &[
    CssCoverageArea::ParserValues,
    CssCoverageArea::Selectors,
    CssCoverageArea::Shorthands,
    CssCoverageArea::SupportsMedia,
    CssCoverageArea::ImportResolution,
    CssCoverageArea::ScopeBounds,
    CssCoverageArea::CustomProperties,
];

pub fn runnable_case_metas() -> Vec<CssCaseMeta> {
    let mut metas = Vec::new();
    metas.extend(THEME_CSS_PARSER_FIXTURES.iter().map(|fixture| fixture.meta));
    metas.extend(
        THEME_CSS_STYLESHEET_FIXTURES
            .iter()
            .map(|fixture| fixture.meta),
    );
    metas.extend(STYLE_ENGINE_FIXTURES.iter().map(|fixture| fixture.meta));
    metas
}

pub fn all_case_metas() -> Vec<CssCaseMeta> {
    let mut metas = runnable_case_metas();
    metas.extend(EXTERNAL_CSS_COVERAGE.iter().map(|fixture| fixture.meta));
    metas
}
