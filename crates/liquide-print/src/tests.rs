//! Tests for the print system.

use crate::*;

// ── Paper size tests ──────────────────────────────────────────────────

#[test]
fn paper_a4_dimensions() {
    assert_eq!(PAPER_A4.width_mm, 210.0);
    assert_eq!(PAPER_A4.height_mm, 297.0);
    assert_eq!(PAPER_A4.name(), "A4");
}

#[test]
fn paper_letter_dimensions() {
    assert!((PAPER_LETTER.width_mm - 215.9).abs() < 0.01);
    assert!((PAPER_LETTER.height_mm - 279.4).abs() < 0.01);
}

#[test]
fn paper_from_name_case_insensitive() {
    let p = PaperSize::from_name("a4").unwrap();
    assert_eq!(p, PAPER_A4);

    let p = PaperSize::from_name("LETTER").unwrap();
    assert_eq!(p, PAPER_LETTER);

    let p = PaperSize::from_name("legal").unwrap();
    assert_eq!(p, PAPER_LEGAL);
}

#[test]
fn paper_from_name_unknown_returns_none() {
    assert!(PaperSize::from_name("Foolscap").is_none());
}

#[test]
fn paper_custom_size() {
    let p = PaperSize::custom("Envelope", 110.0, 220.0);
    assert_eq!(p.name(), "Envelope");
    assert_eq!(p.width_mm, 110.0);
    assert_eq!(p.height_mm, 220.0);
}

#[test]
fn paper_area_and_inches() {
    let area = PAPER_A4.area_mm2();
    assert!((area - 210.0 * 297.0).abs() < 0.1);

    let (w_in, h_in) = PAPER_A4.dimensions_inches();
    assert!((w_in - 8.2677).abs() < 0.01);
    assert!((h_in - 11.6929).abs() < 0.01);
}

#[test]
fn paper_is_landscape() {
    assert!(!PAPER_A4.is_landscape());
    let wide = PaperSize::custom("Wide", 300.0, 200.0);
    assert!(wide.is_landscape());
}

#[test]
fn paper_equality() {
    let a = PaperSize::custom("X", 210.0, 297.0);
    assert_eq!(a, PAPER_A4);

    let b = PaperSize::custom("Y", 210.0, 300.0);
    assert_ne!(b, PAPER_A4);
}

// ── Printer tests ─────────────────────────────────────────────────────

#[test]
fn printer_status_is_ready() {
    assert!(PrinterStatus::Idle.is_ready());
    assert!(!PrinterStatus::Printing.is_ready());
    assert!(!PrinterStatus::Offline.is_ready());
    assert!(!PrinterStatus::Error("jam".into()).is_ready());
    assert!(!PrinterStatus::PaperJam.is_ready());
    assert!(!PrinterStatus::LowToner.is_ready());
}

#[test]
fn printer_status_labels() {
    assert_eq!(PrinterStatus::Idle.label(), "Idle");
    assert_eq!(PrinterStatus::PaperJam.label(), "Paper Jam");
    assert_eq!(PrinterStatus::LowToner.label(), "Low Toner");
}

#[test]
fn printer_capabilities_default() {
    let caps = PrinterCapabilities::default();
    assert!(caps.supports_color);
    assert!(!caps.supports_duplex);
    assert_eq!(caps.max_dpi, 600);
    assert_eq!(caps.max_copies, 99);
    assert_eq!(caps.paper_sizes.len(), 2);
}

#[test]
fn printer_capabilities_supports_paper() {
    let caps = PrinterCapabilities::default();
    assert!(caps.supports_paper(&PAPER_A4));
    assert!(caps.supports_paper(&PAPER_LETTER));
    assert!(!caps.supports_paper(&PAPER_TABLOID));
}

#[test]
fn printer_capabilities_supports_dpi() {
    let caps = PrinterCapabilities::default();
    assert!(caps.supports_dpi(300));
    assert!(caps.supports_dpi(600));
    assert!(!caps.supports_dpi(1200));
}

// ── Settings tests ────────────────────────────────────────────────────

#[test]
fn margins_default() {
    let m = Margins::default();
    assert!((m.top_mm - 25.4).abs() < 0.01);
    assert!((m.horizontal() - 50.8).abs() < 0.01);
    assert!((m.vertical() - 50.8).abs() < 0.01);
}

#[test]
fn margins_narrow() {
    let m = Margins::narrow();
    assert!((m.left_mm - 12.7).abs() < 0.01);
    assert!((m.right_mm - 12.7).abs() < 0.01);
}

#[test]
fn margins_none() {
    let m = Margins::none();
    assert_eq!(m.top_mm, 0.0);
    assert_eq!(m.horizontal(), 0.0);
}

#[test]
fn page_range_all() {
    let pages = PageRange::All.resolve(10);
    assert_eq!(pages, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
}

#[test]
fn page_range_range() {
    let pages = PageRange::Range(3, 7).resolve(10);
    assert_eq!(pages, vec![3, 4, 5, 6, 7]);
}

#[test]
fn page_range_range_clamped() {
    let pages = PageRange::Range(8, 15).resolve(10);
    assert_eq!(pages, vec![8, 9, 10]);
}

#[test]
fn page_range_range_empty() {
    let pages = PageRange::Range(5, 3).resolve(10);
    assert!(pages.is_empty());
}

#[test]
fn page_range_pages() {
    let pages = PageRange::Pages(vec![5, 2, 2, 9, 100]).resolve(10);
    assert_eq!(pages, vec![2, 5, 9]);
}

#[test]
fn print_scale_factor() {
    assert_eq!(PrintScale::ActualSize.factor(), 1.0);
    assert_eq!(PrintScale::FitToPage.factor(), 1.0);
    assert!((PrintScale::Custom(0.75).factor() - 0.75).abs() < 0.001);
}

#[test]
fn default_print_settings() {
    let s = PrintSettings::default();
    assert_eq!(s.copies, 1);
    assert_eq!(s.orientation, Orientation::Portrait);
    assert_eq!(s.duplex, DuplexMode::None);
    assert_eq!(s.color_mode, ColorMode::Color);
    assert_eq!(s.page_range, PageRange::All);
}

// ── Job tests ─────────────────────────────────────────────────────────

#[test]
fn job_status_terminal() {
    assert!(!JobStatus::Queued.is_terminal());
    assert!(!JobStatus::Printing.is_terminal());
    assert!(JobStatus::Completed.is_terminal());
    assert!(JobStatus::Cancelled.is_terminal());
    assert!(JobStatus::Failed("err".into()).is_terminal());
}

#[test]
fn job_progress() {
    let job = PrintJob {
        id: 1,
        printer_id: PrinterId(1),
        document_name: "test.pdf".into(),
        settings: PrintSettings::default(),
        status: JobStatus::Printing,
        pages_printed: 5,
        total_pages: 10,
        created_at: 1000,
        started_at: Some(1100),
        completed_at: None,
    };
    assert!((job.progress() - 0.5).abs() < 0.01);
    assert!(job.is_active());
    assert!(job.duration_us().is_none());
}

#[test]
fn job_progress_zero_pages() {
    let job = PrintJob {
        id: 1,
        printer_id: PrinterId(1),
        document_name: "empty.pdf".into(),
        settings: PrintSettings::default(),
        status: JobStatus::Queued,
        pages_printed: 0,
        total_pages: 0,
        created_at: 1000,
        started_at: None,
        completed_at: None,
    };
    assert_eq!(job.progress(), 0.0);
}

#[test]
fn job_duration() {
    let job = PrintJob {
        id: 1,
        printer_id: PrinterId(1),
        document_name: "doc.pdf".into(),
        settings: PrintSettings::default(),
        status: JobStatus::Completed,
        pages_printed: 3,
        total_pages: 3,
        created_at: 1000,
        started_at: Some(1050),
        completed_at: Some(2000),
    };
    assert_eq!(job.duration_us(), Some(1000));
    assert!(!job.is_active());
}

// ── Layout tests ──────────────────────────────────────────────────────

#[test]
fn printable_area_portrait_default_margins() {
    let area = compute_printable_area(&PAPER_A4, &Margins::default(), Orientation::Portrait);
    assert!((area.x_mm - 25.4).abs() < 0.01);
    assert!((area.y_mm - 25.4).abs() < 0.01);
    assert!((area.width_mm - (210.0 - 50.8)).abs() < 0.01);
    assert!((area.height_mm - (297.0 - 50.8)).abs() < 0.01);
}

#[test]
fn printable_area_landscape() {
    let area = compute_printable_area(&PAPER_A4, &Margins::default(), Orientation::Landscape);
    // In landscape, paper is rotated: width=297, height=210
    assert!((area.width_mm - (297.0 - 50.8)).abs() < 0.01);
    assert!((area.height_mm - (210.0 - 50.8)).abs() < 0.01);
}

#[test]
fn printable_area_no_margins() {
    let area = compute_printable_area(&PAPER_A4, &Margins::none(), Orientation::Portrait);
    assert_eq!(area.x_mm, 0.0);
    assert_eq!(area.y_mm, 0.0);
    assert_eq!(area.width_mm, 210.0);
    assert_eq!(area.height_mm, 297.0);
}

#[test]
fn printable_area_extreme_margins_clamp() {
    let huge = Margins {
        top_mm: 200.0,
        bottom_mm: 200.0,
        left_mm: 200.0,
        right_mm: 200.0,
    };
    let area = compute_printable_area(&PAPER_A4, &huge, Orientation::Portrait);
    assert_eq!(area.width_mm, 0.0);
    assert_eq!(area.height_mm, 0.0);
}

#[test]
fn printable_area_aspect_ratio() {
    let area = compute_printable_area(&PAPER_A4, &Margins::none(), Orientation::Portrait);
    let ratio = area.aspect_ratio();
    assert!((ratio - 210.0 / 297.0).abs() < 0.001);
}

#[test]
fn n_up_1_page() {
    let area = PrintableArea {
        x_mm: 25.0,
        y_mm: 25.0,
        width_mm: 160.0,
        height_mm: 247.0,
    };
    let rects = n_up_layout(&area, 1);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].x_mm, 25.0);
    assert_eq!(rects[0].y_mm, 25.0);
    assert_eq!(rects[0].width_mm, 160.0);
    assert_eq!(rects[0].height_mm, 247.0);
}

#[test]
fn n_up_2_pages() {
    let area = PrintableArea {
        x_mm: 0.0,
        y_mm: 0.0,
        width_mm: 200.0,
        height_mm: 100.0,
    };
    let rects = n_up_layout(&area, 2);
    assert_eq!(rects.len(), 2);
    // 2 columns, 1 row, 1mm gap
    let slot_w = (200.0 - 1.0) / 2.0;
    assert!((rects[0].width_mm - slot_w).abs() < 0.01);
    assert!((rects[1].x_mm - (slot_w + 1.0)).abs() < 0.01);
}

#[test]
fn n_up_4_pages() {
    let area = PrintableArea {
        x_mm: 10.0,
        y_mm: 10.0,
        width_mm: 190.0,
        height_mm: 277.0,
    };
    let rects = n_up_layout(&area, 4);
    assert_eq!(rects.len(), 4);
    // 2x2 grid
    let slot_w = (190.0 - 1.0) / 2.0;
    let slot_h = (277.0 - 1.0) / 2.0;
    assert!((rects[0].width_mm - slot_w).abs() < 0.01);
    assert!((rects[0].height_mm - slot_h).abs() < 0.01);
    // Bottom-right slot
    assert!((rects[3].x_mm - (10.0 + slot_w + 1.0)).abs() < 0.01);
    assert!((rects[3].y_mm - (10.0 + slot_h + 1.0)).abs() < 0.01);
}

#[test]
fn n_up_6_pages() {
    let area = PrintableArea {
        x_mm: 0.0,
        y_mm: 0.0,
        width_mm: 300.0,
        height_mm: 200.0,
    };
    let rects = n_up_layout(&area, 6);
    assert_eq!(rects.len(), 6);
    // 3 cols x 2 rows
}

#[test]
fn n_up_9_pages() {
    let area = PrintableArea {
        x_mm: 0.0,
        y_mm: 0.0,
        width_mm: 210.0,
        height_mm: 297.0,
    };
    let rects = n_up_layout(&area, 9);
    assert_eq!(rects.len(), 9);
    // 3x3 grid
}

#[test]
fn n_up_invalid_falls_back_to_1() {
    let area = PrintableArea {
        x_mm: 0.0,
        y_mm: 0.0,
        width_mm: 100.0,
        height_mm: 100.0,
    };
    let rects = n_up_layout(&area, 7);
    assert_eq!(rects.len(), 1);
}

// ── Manager tests ─────────────────────────────────────────────────────

fn make_test_printer(id: u64, name: &str, is_default: bool) -> Printer {
    Printer {
        id: PrinterId(id),
        name: name.to_string(),
        location: Some("Office".to_string()),
        driver: "TestDriver".to_string(),
        status: PrinterStatus::Idle,
        capabilities: PrinterCapabilities::default(),
        is_default,
        is_network: false,
    }
}

#[test]
fn manager_add_and_find_printer() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "HP LaserJet", true));
    mgr.add_printer(make_test_printer(2, "Epson Inkjet", false));

    assert_eq!(mgr.printers().len(), 2);
    assert!(mgr.printer_by_id(PrinterId(1)).is_some());
    assert!(mgr.printer_by_id(PrinterId(99)).is_none());
}

#[test]
fn manager_default_printer() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "HP LaserJet", false));
    mgr.add_printer(make_test_printer(2, "Epson Inkjet", true));

    let def = mgr.default_printer().unwrap();
    assert_eq!(def.name, "Epson Inkjet");
}

#[test]
fn manager_submit_job() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "TestPrinter", true));

    let id = mgr
        .submit_job(PrinterId(1), "document.pdf", PrintSettings::default(), 5)
        .unwrap();
    assert_eq!(id, 1);

    let job = mgr.job_status(id).unwrap();
    assert_eq!(job.document_name, "document.pdf");
    assert_eq!(job.total_pages, 5);
    assert!(matches!(job.status, JobStatus::Queued));
}

#[test]
fn manager_submit_to_unknown_printer_returns_none() {
    let mut mgr = PrintManager::new();
    let result = mgr.submit_job(PrinterId(99), "doc.pdf", PrintSettings::default(), 1);
    assert!(result.is_none());
}

#[test]
fn manager_cancel_job() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));
    let id = mgr
        .submit_job(PrinterId(1), "doc.pdf", PrintSettings::default(), 10)
        .unwrap();

    mgr.cancel_job(id);
    let job = mgr.job_status(id).unwrap();
    assert!(matches!(job.status, JobStatus::Cancelled));
    assert!(job.completed_at.is_some());
}

#[test]
fn manager_job_lifecycle() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));
    let id = mgr
        .submit_job(PrinterId(1), "doc.pdf", PrintSettings::default(), 3)
        .unwrap();

    // Start
    mgr.start_job(id);
    assert!(matches!(
        mgr.job_status(id).unwrap().status,
        JobStatus::Printing
    ));

    // Advance pages
    mgr.advance_page(id);
    assert_eq!(mgr.job_status(id).unwrap().pages_printed, 1);
    mgr.advance_page(id);
    assert_eq!(mgr.job_status(id).unwrap().pages_printed, 2);
    mgr.advance_page(id);

    // Should auto-complete
    let job = mgr.job_status(id).unwrap();
    assert!(matches!(job.status, JobStatus::Completed));
    assert_eq!(job.pages_printed, 3);
    assert!(job.completed_at.is_some());
}

#[test]
fn manager_fail_job() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));
    let id = mgr
        .submit_job(PrinterId(1), "doc.pdf", PrintSettings::default(), 10)
        .unwrap();
    mgr.start_job(id);
    mgr.fail_job(id, "Paper tray empty");

    let job = mgr.job_status(id).unwrap();
    assert!(matches!(job.status, JobStatus::Failed(ref msg) if msg == "Paper tray empty"));
}

#[test]
fn manager_active_and_history() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));

    let id1 = mgr
        .submit_job(PrinterId(1), "a.pdf", PrintSettings::default(), 1)
        .unwrap();
    let _id2 = mgr
        .submit_job(PrinterId(1), "b.pdf", PrintSettings::default(), 1)
        .unwrap();

    assert_eq!(mgr.active_jobs().len(), 2);
    assert_eq!(mgr.history().len(), 0);

    mgr.cancel_job(id1);
    assert_eq!(mgr.active_jobs().len(), 1);
    assert_eq!(mgr.history().len(), 1);
}

#[test]
fn manager_clear_history() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));

    let id1 = mgr
        .submit_job(PrinterId(1), "a.pdf", PrintSettings::default(), 1)
        .unwrap();
    let _id2 = mgr
        .submit_job(PrinterId(1), "b.pdf", PrintSettings::default(), 1)
        .unwrap();
    mgr.cancel_job(id1);

    assert_eq!(mgr.total_jobs(), 2);
    mgr.clear_history();
    assert_eq!(mgr.total_jobs(), 1);
}

#[test]
fn manager_multiple_job_ids() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));

    let id1 = mgr
        .submit_job(PrinterId(1), "a.pdf", PrintSettings::default(), 1)
        .unwrap();
    let id2 = mgr
        .submit_job(PrinterId(1), "b.pdf", PrintSettings::default(), 1)
        .unwrap();
    let id3 = mgr
        .submit_job(PrinterId(1), "c.pdf", PrintSettings::default(), 1)
        .unwrap();

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn manager_cancel_already_terminal_is_noop() {
    let mut mgr = PrintManager::new();
    mgr.add_printer(make_test_printer(1, "P", true));
    let id = mgr
        .submit_job(PrinterId(1), "doc.pdf", PrintSettings::default(), 1)
        .unwrap();
    mgr.start_job(id);
    mgr.advance_page(id);
    // Now completed.
    assert!(matches!(
        mgr.job_status(id).unwrap().status,
        JobStatus::Completed
    ));
    // Cancel should be a no-op.
    mgr.cancel_job(id);
    assert!(matches!(
        mgr.job_status(id).unwrap().status,
        JobStatus::Completed
    ));
}

// ── Preset paper sizes ───────────────────────────────────────────────

#[test]
fn all_presets_have_correct_names() {
    assert_eq!(PAPER_A3.name(), "A3");
    assert_eq!(PAPER_A5.name(), "A5");
    assert_eq!(PAPER_B5.name(), "B5");
    assert_eq!(PAPER_LEGAL.name(), "Legal");
    assert_eq!(PAPER_TABLOID.name(), "Tabloid");
}

#[test]
fn from_name_all_presets() {
    for name in ["A4", "A3", "A5", "Letter", "Legal", "Tabloid", "B5"] {
        assert!(
            PaperSize::from_name(name).is_some(),
            "from_name({}) should succeed",
            name
        );
    }
}
