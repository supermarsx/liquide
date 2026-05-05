use std::path::PathBuf;

use liquide_xdg::desktop_entry::{DesktopEntry, EntryType};

use crate::{
    ExecExpansionError, ShellApp, ShellAssociationRegistry, ShellExecuteError, ShellExecuteRequest,
    ShellTarget, ShellVerb, expand_exec_template,
};

fn app(id: &str, name: &str, exec: &str) -> ShellApp {
    ShellApp::new(
        id,
        DesktopEntry {
            name: name.to_string(),
            exec: Some(exec.to_string()),
            type_: EntryType::Application,
            ..DesktopEntry::default()
        },
    )
}

fn terminal_app(id: &str, name: &str, exec: &str) -> ShellApp {
    let mut app = app(id, name, exec);
    app.entry.terminal = true;
    app.entry.path = Some("/tmp".to_string());
    app
}

#[test]
fn plans_default_mime_association_for_file() {
    let mut registry = ShellAssociationRegistry::new();
    registry.register_app(app("org.liquide.Text.desktop", "Text", "liquide-text %f"));
    registry.set_default_mime_handler("text/plain", "org.liquide.Text.desktop");

    let plan = registry
        .plan_execute(ShellExecuteRequest::open(ShellTarget::File(PathBuf::from(
            "/home/mariana/readme.txt",
        ))))
        .unwrap();

    assert_eq!(plan.app_id, "org.liquide.Text.desktop");
    assert_eq!(plan.app_name, "Text");
    assert_eq!(
        plan.command,
        vec!["liquide-text", "/home/mariana/readme.txt"]
    );
    assert_eq!(plan.mime_type.unwrap().essence(), "text/plain");
    assert!(!plan.terminal);
}

#[test]
fn reports_missing_association_for_unhandled_file() {
    let registry = ShellAssociationRegistry::new();

    let err = registry
        .plan_execute(ShellExecuteRequest::open(ShellTarget::File(PathBuf::from(
            "/home/mariana/readme.txt",
        ))))
        .unwrap_err();

    assert_eq!(
        err,
        ShellExecuteError::NoAssociation {
            mime_type: Some("text/plain".to_string()),
            target: "file:///home/mariana/readme.txt".to_string(),
        }
    );
}

#[test]
fn open_with_override_skips_default_lookup() {
    let mut registry = ShellAssociationRegistry::new();
    registry.register_app(app("org.liquide.Hex.desktop", "Hex", "hex-viewer %f"));

    let plan = registry
        .plan_execute(ShellExecuteRequest::open_with(
            ShellTarget::File(PathBuf::from("/tmp/blob.bin")),
            "org.liquide.Hex.desktop",
        ))
        .unwrap();

    assert_eq!(plan.app_id, "org.liquide.Hex.desktop");
    assert_eq!(plan.command, vec!["hex-viewer", "/tmp/blob.bin"]);
}

#[test]
fn plans_uri_with_scheme_handler() {
    let mut registry = ShellAssociationRegistry::new();
    registry.register_app(app("org.liquide.Browser.desktop", "Browser", "browser %u"));
    registry.set_scheme_handler("https", "org.liquide.Browser.desktop");

    let plan = registry
        .plan_execute(ShellExecuteRequest::open(ShellTarget::Uri(
            "https://example.test/path".to_string(),
        )))
        .unwrap();

    assert_eq!(plan.command, vec!["browser", "https://example.test/path"]);
    assert!(plan.mime_type.is_none());
}

#[test]
fn preserves_terminal_flag_and_working_directory() {
    let mut registry = ShellAssociationRegistry::new();
    registry.register_app(terminal_app(
        "org.liquide.TerminalEditor.desktop",
        "Terminal Editor",
        "nano %f",
    ));
    registry.set_default_mime_handler("text/plain", "org.liquide.TerminalEditor.desktop");

    let plan = registry
        .plan_execute(ShellExecuteRequest::open(ShellTarget::File(PathBuf::from(
            "/tmp/notes.txt",
        ))))
        .unwrap();

    assert!(plan.terminal);
    assert_eq!(plan.working_directory, Some(PathBuf::from("/tmp")));
}

#[test]
fn expands_exec_field_codes() {
    let app = app("org.liquide.Viewer.desktop", "Liquide Viewer", "viewer");
    let args = expand_exec_template(
        "viewer --name %c --desktop %k %% %u",
        &app,
        &[ShellTarget::Uri("trash:///readme.txt".to_string())],
    )
    .unwrap();

    assert_eq!(
        args,
        vec![
            "viewer",
            "--name",
            "Liquide Viewer",
            "--desktop",
            "org.liquide.Viewer.desktop",
            "%",
            "trash:///readme.txt",
        ]
    );
}

#[test]
fn expands_multi_file_field_as_multiple_arguments() {
    let app = app("org.liquide.Archive.desktop", "Archive", "archive %F");
    let args = expand_exec_template(
        app.entry.exec.as_deref().unwrap(),
        &app,
        &[
            ShellTarget::File(PathBuf::from("/tmp/a.txt")),
            ShellTarget::File(PathBuf::from("/tmp/b.txt")),
        ],
    )
    .unwrap();

    assert_eq!(args, vec!["archive", "/tmp/a.txt", "/tmp/b.txt"]);
}

#[test]
fn quoted_exec_tokens_preserve_spaces() {
    let app = app("org.liquide.Text.desktop", "Text", "text-editor");
    let args = expand_exec_template(
        "text-editor --label 'Project Notes' --path \"%f\"",
        &app,
        &[ShellTarget::File(PathBuf::from(
            "/home/mariana/project notes.txt",
        ))],
    )
    .unwrap();

    assert_eq!(
        args,
        vec![
            "text-editor",
            "--label",
            "Project Notes",
            "--path",
            "/home/mariana/project notes.txt",
        ]
    );
}

#[test]
fn file_field_rejects_uri_target() {
    let app = app("org.liquide.Editor.desktop", "Editor", "editor %f");

    let err = expand_exec_template(
        app.entry.exec.as_deref().unwrap(),
        &app,
        &[ShellTarget::Uri("https://example.test".to_string())],
    )
    .unwrap_err();

    assert_eq!(err, ExecExpansionError::RequiresFileTarget { field: 'f' });
}

#[test]
fn unsupported_verbs_are_not_planned_yet() {
    let registry = ShellAssociationRegistry::new();

    let err = registry
        .plan_execute(ShellExecuteRequest {
            targets: vec![ShellTarget::File(PathBuf::from("/tmp/notes.txt"))],
            verb: ShellVerb::Print,
            app_id_override: None,
        })
        .unwrap_err();

    assert_eq!(
        err,
        ShellExecuteError::UnsupportedVerb {
            verb: "print".to_string(),
        }
    );
}
