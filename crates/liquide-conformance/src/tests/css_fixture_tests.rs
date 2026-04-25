use std::collections::{HashMap, HashSet};

use crate::css::{
    CssCoverageArea, EXTERNAL_CSS_COVERAGE, REQUIRED_RUNNABLE_AREAS, all_case_metas,
    runnable_case_metas,
};

#[test]
fn css_fixture_ids_are_unique() {
    let all = all_case_metas();
    let unique: HashSet<_> = all.iter().map(|meta| meta.id).collect();
    assert_eq!(unique.len(), all.len());
}

#[test]
fn css_fixture_catalog_covers_required_runnable_areas() {
    let areas: HashSet<_> = runnable_case_metas()
        .into_iter()
        .map(|meta| meta.area)
        .collect();

    for area in REQUIRED_RUNNABLE_AREAS {
        assert!(
            areas.contains(area),
            "missing runnable CSS conformance coverage for {:?}",
            area
        );
    }
}

#[test]
fn each_runnable_area_has_negative_or_recovery_coverage() {
    let mut kinds_by_area: HashMap<CssCoverageArea, Vec<_>> = HashMap::new();
    for meta in runnable_case_metas() {
        kinds_by_area.entry(meta.area).or_default().push(meta.kind);
    }

    for area in REQUIRED_RUNNABLE_AREAS {
        let kinds = kinds_by_area
            .get(area)
            .unwrap_or_else(|| panic!("missing fixture kinds for {:?}", area));
        assert!(
            kinds.iter().any(|kind| !matches!(kind, crate::css::CssCaseKind::Positive)),
            "expected at least one negative or recovery case for {:?}",
            area
        );
    }
}

#[test]
fn theme_runtime_gap_is_tracked_explicitly() {
    assert_eq!(EXTERNAL_CSS_COVERAGE.len(), 1);
    let theme_runtime = &EXTERNAL_CSS_COVERAGE[0];
    assert_eq!(theme_runtime.meta.area, CssCoverageArea::ThemeRuntime);
    assert!(theme_runtime.validating_suite.contains("liquide-theme-engine"));
    assert!(theme_runtime.note.contains("outside this executor's writable test scope"));
}