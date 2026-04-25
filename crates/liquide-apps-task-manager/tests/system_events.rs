//! Tests for the system event viewer module.

use liquide_apps_task_manager::system_events::*;

// ---------------------------------------------------------------------------
// EventLogSource
// ---------------------------------------------------------------------------

#[test]
fn event_log_source_all_variants() {
    let variants = [
        EventLogSource::Application,
        EventLogSource::System,
        EventLogSource::Security,
        EventLogSource::Setup,
        EventLogSource::ForwardedEvents,
        EventLogSource::Hardware,
        EventLogSource::Custom,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn event_log_source_display() {
    assert_eq!(EventLogSource::Application.as_str(), "Application");
    assert_eq!(EventLogSource::System.as_str(), "System");
    assert_eq!(EventLogSource::Security.as_str(), "Security");
    assert_eq!(EventLogSource::ForwardedEvents.as_str(), "Forwarded Events");
    assert_eq!(EventLogSource::Hardware.as_str(), "Hardware");
    assert_eq!(format!("{}", EventLogSource::Setup), "Setup");
}

#[test]
fn event_log_source_serde_roundtrip() {
    let val = EventLogSource::Security;
    let json = serde_json::to_string(&val).unwrap();
    let back: EventLogSource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// EventLevel
// ---------------------------------------------------------------------------

#[test]
fn event_level_all_variants() {
    let variants = [
        EventLevel::Verbose,
        EventLevel::Information,
        EventLevel::Warning,
        EventLevel::Error,
        EventLevel::Critical,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn event_level_display() {
    assert_eq!(EventLevel::Verbose.as_str(), "Verbose");
    assert_eq!(EventLevel::Information.as_str(), "Information");
    assert_eq!(EventLevel::Warning.as_str(), "Warning");
    assert_eq!(EventLevel::Error.as_str(), "Error");
    assert_eq!(EventLevel::Critical.as_str(), "Critical");
    assert_eq!(format!("{}", EventLevel::Critical), "Critical");
}

#[test]
fn event_level_severity_ordering() {
    assert!(EventLevel::Critical.severity() > EventLevel::Error.severity());
    assert!(EventLevel::Error.severity() > EventLevel::Warning.severity());
    assert!(EventLevel::Warning.severity() > EventLevel::Information.severity());
    assert!(EventLevel::Information.severity() > EventLevel::Verbose.severity());
}

#[test]
fn event_level_serde_roundtrip() {
    let val = EventLevel::Warning;
    let json = serde_json::to_string(&val).unwrap();
    let back: EventLevel = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// EventCategory
// ---------------------------------------------------------------------------

#[test]
fn event_category_all_variants() {
    let variants = [
        EventCategory::None,
        EventCategory::Disk,
        EventCategory::Network,
        EventCategory::Printer,
        EventCategory::Security,
        EventCategory::ServiceControl,
        EventCategory::Shell,
        EventCategory::Power,
        EventCategory::Driver,
        EventCategory::Update,
        EventCategory::ApplicationError,
    ];
    assert_eq!(variants.len(), 11);
}

#[test]
fn event_category_display() {
    assert_eq!(EventCategory::ServiceControl.as_str(), "Service Control");
    assert_eq!(
        EventCategory::ApplicationError.as_str(),
        "Application Error"
    );
    assert_eq!(format!("{}", EventCategory::Disk), "Disk");
}

// ---------------------------------------------------------------------------
// SystemEvent
// ---------------------------------------------------------------------------

#[test]
fn system_event_default() {
    let evt = SystemEvent::default();
    assert_eq!(evt.record_id, 0);
    assert_eq!(evt.event_id, 0);
    assert_eq!(evt.level, EventLevel::Information);
    assert_eq!(evt.source, EventLogSource::Application);
    assert_eq!(evt.category, EventCategory::None);
    assert!(evt.message.is_empty());
    assert!(evt.keywords.is_empty());
    assert!(evt.user.is_none());
    assert!(evt.process_id.is_none());
    assert!(evt.activity_id.is_none());
}

#[test]
fn system_event_construction() {
    let evt = SystemEvent {
        record_id: 42,
        timestamp: "2026-01-15T10:30:00Z".into(),
        source: EventLogSource::System,
        provider: "Microsoft-Windows-Kernel-Power".into(),
        event_id: 41,
        level: EventLevel::Critical,
        category: EventCategory::Power,
        task: Some("Unexpected Shutdown".into()),
        message: "The system has rebooted without cleanly shutting down first.".into(),
        raw_data: None,
        user: Some("SYSTEM".into()),
        computer: "DESKTOP-ABC123".into(),
        process_id: Some(4),
        thread_id: Some(8),
        keywords: vec!["kernel".into(), "power".into()],
        activity_id: None,
    };
    assert_eq!(evt.record_id, 42);
    assert_eq!(evt.event_id, 41);
    assert_eq!(evt.level, EventLevel::Critical);
    assert_eq!(evt.source, EventLogSource::System);
    assert_eq!(evt.keywords.len(), 2);
}

#[test]
fn system_event_serde_roundtrip() {
    let evt = SystemEvent {
        record_id: 100,
        timestamp: "2026-02-01T08:00:00Z".into(),
        source: EventLogSource::Application,
        provider: "MyApp".into(),
        event_id: 1001,
        level: EventLevel::Error,
        category: EventCategory::ApplicationError,
        task: None,
        message: "Unhandled exception".into(),
        raw_data: Some("<Event>...</Event>".into()),
        user: Some("DESKTOP\\User".into()),
        computer: "WORKSTATION".into(),
        process_id: Some(1234),
        thread_id: Some(5678),
        keywords: vec!["crash".into()],
        activity_id: Some("abc-123".into()),
    };
    let json = serde_json::to_string(&evt).unwrap();
    let back: SystemEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.record_id, 100);
    assert_eq!(back.event_id, 1001);
    assert_eq!(back.provider, "MyApp");
    assert_eq!(back.activity_id.unwrap(), "abc-123");
}

// ---------------------------------------------------------------------------
// EventLogFilter
// ---------------------------------------------------------------------------

#[test]
fn event_log_filter_default() {
    let filter = EventLogFilter::default();
    assert!(filter.sources.is_none());
    assert!(filter.levels.is_none());
    assert!(filter.event_ids.is_none());
    assert!(filter.providers.is_none());
    assert!(filter.categories.is_none());
    assert!(filter.from_time.is_none());
    assert!(filter.to_time.is_none());
    assert!(filter.keyword_search.is_none());
    assert!(filter.pid.is_none());
    assert!(filter.max_results.is_none());
}

#[test]
fn event_log_filter_construction() {
    let filter = EventLogFilter {
        sources: Some(vec![EventLogSource::System, EventLogSource::Application]),
        levels: Some(vec![EventLevel::Error, EventLevel::Critical]),
        event_ids: Some(vec![41, 6008]),
        providers: None,
        categories: None,
        from_time: Some("2026-01-01T00:00:00Z".into()),
        to_time: None,
        keyword_search: Some("shutdown".into()),
        pid: None,
        max_results: Some(500),
    };
    assert_eq!(filter.sources.as_ref().unwrap().len(), 2);
    assert_eq!(filter.levels.as_ref().unwrap().len(), 2);
    assert_eq!(filter.max_results.unwrap(), 500);
}

#[test]
fn event_log_filter_serde_roundtrip() {
    let filter = EventLogFilter {
        sources: Some(vec![EventLogSource::Security]),
        levels: Some(vec![EventLevel::Warning]),
        event_ids: None,
        providers: Some(vec!["logon".into()]),
        categories: None,
        from_time: None,
        to_time: None,
        keyword_search: None,
        pid: Some(1234),
        max_results: Some(100),
    };
    let json = serde_json::to_string(&filter).unwrap();
    let back: EventLogFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(back.sources.as_ref().unwrap()[0], EventLogSource::Security);
    assert_eq!(back.pid.unwrap(), 1234);
}

// ---------------------------------------------------------------------------
// EventLogStats
// ---------------------------------------------------------------------------

#[test]
fn event_log_stats_construction() {
    let stats = EventLogStats {
        total_events: 50000,
        critical_count: 5,
        error_count: 150,
        warning_count: 800,
        information_count: 40000,
        verbose_count: 9045,
        oldest_event: Some("2025-06-01T00:00:00Z".into()),
        newest_event: Some("2026-02-12T12:00:00Z".into()),
        log_size_bytes: 52428800,
        max_log_size_bytes: 104857600,
    };
    assert_eq!(stats.total_events, 50000);
    assert_eq!(stats.critical_count, 5);
    assert!(stats.oldest_event.is_some());
}

// ---------------------------------------------------------------------------
// EventLogView
// ---------------------------------------------------------------------------

#[test]
fn event_log_view_all_variants() {
    let variants = [
        EventLogView::All,
        EventLogView::Application,
        EventLogView::System,
        EventLogView::Security,
        EventLogView::Setup,
        EventLogView::Hardware,
        EventLogView::Summary,
        EventLogView::Bookmarks,
        EventLogView::CustomFilter,
    ];
    assert_eq!(variants.len(), 9);
}

#[test]
fn event_log_view_display() {
    assert_eq!(EventLogView::All.as_str(), "All Events");
    assert_eq!(EventLogView::Summary.as_str(), "Summary");
    assert_eq!(EventLogView::CustomFilter.as_str(), "Custom Filter");
    assert_eq!(format!("{}", EventLogView::Bookmarks), "Bookmarks");
}

// ---------------------------------------------------------------------------
// EventBookmark
// ---------------------------------------------------------------------------

#[test]
fn event_bookmark_construction() {
    let bm = EventBookmark {
        record_id: 42,
        source: EventLogSource::System,
        note: Some("Investigate this crash".into()),
        created_at: "2026-02-12T10:00:00Z".into(),
    };
    assert_eq!(bm.record_id, 42);
    assert_eq!(bm.source, EventLogSource::System);
    assert!(bm.note.is_some());
}

#[test]
fn event_bookmark_serde_roundtrip() {
    let bm = EventBookmark {
        record_id: 99,
        source: EventLogSource::Application,
        note: None,
        created_at: "2026-02-12T10:00:00Z".into(),
    };
    let json = serde_json::to_string(&bm).unwrap();
    let back: EventBookmark = serde_json::from_str(&json).unwrap();
    assert_eq!(back.record_id, 99);
    assert!(back.note.is_none());
}

// ---------------------------------------------------------------------------
// EventExportFormat
// ---------------------------------------------------------------------------

#[test]
fn event_export_format_all_variants() {
    let variants = [
        EventExportFormat::Csv,
        EventExportFormat::Json,
        EventExportFormat::Xml,
        EventExportFormat::Evtx,
        EventExportFormat::Html,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn event_export_format_display() {
    assert_eq!(EventExportFormat::Csv.as_str(), "CSV");
    assert_eq!(EventExportFormat::Evtx.as_str(), "EVTX");
    assert_eq!(format!("{}", EventExportFormat::Html), "HTML");
}

// ---------------------------------------------------------------------------
// EventCorrelation
// ---------------------------------------------------------------------------

#[test]
fn event_correlation_construction() {
    let corr = EventCorrelation {
        activity_id: "abc-def-123".into(),
        event_record_ids: vec![10, 11, 12, 13],
        description: Some("Service restart sequence".into()),
    };
    assert_eq!(corr.event_record_ids.len(), 4);
    assert!(corr.description.is_some());
}

// ---------------------------------------------------------------------------
// EventAction
// ---------------------------------------------------------------------------

#[test]
fn event_action_all_variants() {
    let variants = [
        EventAction::ViewDetails,
        EventAction::Bookmark,
        EventAction::RemoveBookmark,
        EventAction::CopyMessage,
        EventAction::CopyRawData,
        EventAction::LookupOnline,
        EventAction::CreateFilter,
        EventAction::ClearLog,
        EventAction::ExportSelected,
        EventAction::AttachTask,
    ];
    assert_eq!(variants.len(), 10);
}

#[test]
fn event_action_display() {
    assert_eq!(EventAction::ViewDetails.as_str(), "View Details");
    assert_eq!(EventAction::LookupOnline.as_str(), "Lookup Online");
    assert_eq!(EventAction::AttachTask.as_str(), "Attach Task");
    assert_eq!(format!("{}", EventAction::ClearLog), "Clear Log");
}

#[test]
fn event_action_serde_roundtrip() {
    let val = EventAction::Bookmark;
    let json = serde_json::to_string(&val).unwrap();
    let back: EventAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// Config integration
// ---------------------------------------------------------------------------

#[test]
fn default_system_events_config() {
    use liquide_apps_task_manager::config::SystemEventsConfig;
    let cfg = SystemEventsConfig::default();
    assert_eq!(cfg.default_view, "all");
    assert_eq!(cfg.max_events_loaded, 10000);
    assert_eq!(cfg.auto_refresh_ms, 5000);
    assert!(!cfg.show_verbose);
    assert!(cfg.show_information);
    assert!(cfg.show_warnings);
    assert!(cfg.show_errors);
    assert!(cfg.show_critical);
    assert!(cfg.notify_critical);
    assert!(!cfg.notify_errors);
    assert_eq!(cfg.default_hours_range, 24);
    assert!(cfg.resolve_sids);
}

#[test]
fn task_manager_config_has_system_events() {
    use liquide_apps_task_manager::config::TaskManagerConfig;
    let cfg = TaskManagerConfig::default();
    assert_eq!(cfg.system_events.max_events_loaded, 10000);
}

// ---------------------------------------------------------------------------
// Tab integration
// ---------------------------------------------------------------------------

#[test]
fn tab_id_includes_system_event_viewer() {
    use liquide_apps_task_manager::ui::TabId;
    let tab = TabId::SystemEventViewer;
    assert_eq!(tab.as_str(), "Event Viewer");
    assert_eq!(format!("{tab}"), "Event Viewer");
}

// ---------------------------------------------------------------------------
// Event integration
// ---------------------------------------------------------------------------

#[test]
fn event_system_event_log_alert() {
    use liquide_apps_task_manager::event::TaskManagerEvent;
    let evt = TaskManagerEvent::SystemEventLogAlert {
        source: "System".into(),
        event_id: 41,
        message: "Unexpected shutdown".into(),
    };
    assert_eq!(evt.as_str(), "System Event Log Alert");
}

#[test]
fn event_event_log_cleared() {
    use liquide_apps_task_manager::event::TaskManagerEvent;
    let evt = TaskManagerEvent::EventLogCleared {
        source: "Application".into(),
    };
    assert_eq!(evt.as_str(), "Event Log Cleared");
}

// ---------------------------------------------------------------------------
// IPC integration
// ---------------------------------------------------------------------------

#[test]
fn ipc_list_system_events() {
    use liquide_apps_task_manager::ipc::IpcRequest;
    let req = IpcRequest::ListSystemEvents { filter_json: None };
    assert_eq!(req.as_str(), "List System Events");
}

#[test]
fn ipc_get_event_log_stats() {
    use liquide_apps_task_manager::ipc::IpcRequest;
    let req = IpcRequest::GetEventLogStats;
    assert_eq!(req.as_str(), "Get Event Log Stats");
}

#[test]
fn ipc_clear_event_log() {
    use liquide_apps_task_manager::ipc::IpcRequest;
    let req = IpcRequest::ClearEventLog {
        source: "System".into(),
    };
    assert_eq!(req.as_str(), "Clear Event Log");
}
