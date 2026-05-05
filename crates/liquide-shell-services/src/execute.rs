use std::collections::HashMap;
use std::path::{Path, PathBuf};

use liquide_xdg::desktop_entry::DesktopEntry;
use liquide_xdg::mime::{MimeDatabase, MimeType};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A target the shell can operate on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellTarget {
    /// A local filesystem path.
    File(PathBuf),
    /// A URI such as `https://example.com` or `trash:///item`.
    Uri(String),
}

impl ShellTarget {
    /// Return the target as an argument suitable for URI field codes.
    #[must_use]
    pub fn as_uri_argument(&self) -> String {
        match self {
            Self::File(path) => file_uri(path),
            Self::Uri(uri) => uri.clone(),
        }
    }

    /// Return the local path if this is a file target.
    #[must_use]
    pub fn as_file_argument(&self) -> Option<String> {
        match self {
            Self::File(path) => Some(path.to_string_lossy().into_owned()),
            Self::Uri(_) => None,
        }
    }

    fn scheme(&self) -> Option<String> {
        match self {
            Self::File(_) => None,
            Self::Uri(uri) => uri
                .split_once(':')
                .map(|(scheme, _)| scheme.to_ascii_lowercase()),
        }
    }
}

/// A shell verb requested for a target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellVerb {
    Open,
    Edit,
    Print,
    Properties,
    Custom(String),
}

impl ShellVerb {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Edit => "edit",
            Self::Print => "print",
            Self::Properties => "properties",
            Self::Custom(verb) => verb.as_str(),
        }
    }
}

/// Request to plan a shell action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellExecuteRequest {
    pub targets: Vec<ShellTarget>,
    pub verb: ShellVerb,
    pub app_id_override: Option<String>,
}

impl ShellExecuteRequest {
    #[must_use]
    pub fn open(target: ShellTarget) -> Self {
        Self {
            targets: vec![target],
            verb: ShellVerb::Open,
            app_id_override: None,
        }
    }

    #[must_use]
    pub fn open_with(target: ShellTarget, app_id: impl Into<String>) -> Self {
        Self {
            targets: vec![target],
            verb: ShellVerb::Open,
            app_id_override: Some(app_id.into()),
        }
    }
}

/// Registered application metadata used by shell execute planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellApp {
    pub id: String,
    pub entry: DesktopEntry,
}

impl ShellApp {
    #[must_use]
    pub fn new(id: impl Into<String>, entry: DesktopEntry) -> Self {
        Self {
            id: id.into(),
            entry,
        }
    }
}

/// A pure, spawn-free command plan produced by shell execute resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellExecutePlan {
    pub app_id: String,
    pub app_name: String,
    pub verb: ShellVerb,
    pub targets: Vec<ShellTarget>,
    pub command: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub terminal: bool,
    pub mime_type: Option<MimeType>,
}

/// In-memory association and app registry.
pub struct ShellAssociationRegistry {
    mime_database: MimeDatabase,
    apps: HashMap<String, ShellApp>,
    default_mime_handlers: HashMap<String, String>,
    scheme_handlers: HashMap<String, String>,
}

impl ShellAssociationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mime_database: MimeDatabase::new(),
            apps: HashMap::new(),
            default_mime_handlers: HashMap::new(),
            scheme_handlers: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_mime_database(mime_database: MimeDatabase) -> Self {
        Self {
            mime_database,
            ..Self::new()
        }
    }

    pub fn register_app(&mut self, app: ShellApp) {
        self.apps.insert(app.id.clone(), app);
    }

    pub fn set_default_mime_handler(&mut self, mime_type: &str, app_id: impl Into<String>) {
        self.default_mime_handlers
            .insert(mime_type.to_string(), app_id.into());
    }

    pub fn set_scheme_handler(&mut self, scheme: &str, app_id: impl Into<String>) {
        self.scheme_handlers
            .insert(scheme.to_ascii_lowercase(), app_id.into());
    }

    pub fn plan_execute(
        &self,
        request: ShellExecuteRequest,
    ) -> Result<ShellExecutePlan, ShellExecuteError> {
        if request.targets.is_empty() {
            return Err(ShellExecuteError::NoTargets);
        }
        if request.verb != ShellVerb::Open {
            return Err(ShellExecuteError::UnsupportedVerb {
                verb: request.verb.as_str().to_string(),
            });
        }

        let mime_type = self.detect_primary_mime(&request.targets[0]);
        let app_id = match request.app_id_override.clone() {
            Some(app_id) => app_id,
            None => self.resolve_default_app(&request.targets[0], mime_type.as_ref())?,
        };
        let app = self
            .apps
            .get(&app_id)
            .ok_or_else(|| ShellExecuteError::UnknownApplication {
                app_id: app_id.clone(),
            })?;
        let exec = app
            .entry
            .exec
            .as_deref()
            .ok_or_else(|| ShellExecuteError::MissingExec {
                app_id: app_id.clone(),
            })?;
        let command = expand_exec_template(exec, app, &request.targets)?;
        if command.is_empty() {
            return Err(ShellExecuteError::EmptyCommand { app_id });
        }

        Ok(ShellExecutePlan {
            app_id: app.id.clone(),
            app_name: app.entry.name.clone(),
            verb: request.verb,
            targets: request.targets,
            command,
            working_directory: app.entry.path.as_ref().map(PathBuf::from),
            terminal: app.entry.terminal,
            mime_type,
        })
    }

    fn resolve_default_app(
        &self,
        target: &ShellTarget,
        mime_type: Option<&MimeType>,
    ) -> Result<String, ShellExecuteError> {
        if let Some(scheme) = target.scheme() {
            return self.scheme_handlers.get(&scheme).cloned().ok_or_else(|| {
                ShellExecuteError::NoSchemeHandler {
                    scheme,
                    target: target.as_uri_argument(),
                }
            });
        }

        let Some(mime_type) = mime_type else {
            return Err(ShellExecuteError::NoAssociation {
                mime_type: None,
                target: target.as_uri_argument(),
            });
        };
        let essence = mime_type.essence();
        self.default_mime_handlers
            .get(&essence)
            .cloned()
            .ok_or_else(|| ShellExecuteError::NoAssociation {
                mime_type: Some(essence),
                target: target.as_uri_argument(),
            })
    }

    fn detect_primary_mime(&self, target: &ShellTarget) -> Option<MimeType> {
        let ShellTarget::File(path) = target else {
            return None;
        };
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.mime_database.guess_from_extension(ext))
            .or_else(|| Some(MimeType::new("application", "octet-stream")))
    }
}

impl Default for ShellAssociationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShellExecuteError {
    #[error("shell execute requires at least one target")]
    NoTargets,
    #[error("unsupported shell verb: {verb}")]
    UnsupportedVerb { verb: String },
    #[error("unknown application: {app_id}")]
    UnknownApplication { app_id: String },
    #[error("application has no Exec command: {app_id}")]
    MissingExec { app_id: String },
    #[error("application produced an empty command: {app_id}")]
    EmptyCommand { app_id: String },
    #[error("no association for {mime_type:?} target {target}")]
    NoAssociation {
        mime_type: Option<String>,
        target: String,
    },
    #[error("no handler for URI scheme {scheme} target {target}")]
    NoSchemeHandler { scheme: String, target: String },
    #[error(transparent)]
    ExecExpansion(#[from] ExecExpansionError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecExpansionError {
    #[error("unterminated quote in Exec template")]
    UnterminatedQuote,
    #[error("unsupported Exec field code: %{0}")]
    UnsupportedFieldCode(char),
    #[error("Exec field %{field} requires a local file target")]
    RequiresFileTarget { field: char },
    #[error("Exec field %{field} requires a target")]
    RequiresTarget { field: char },
}

/// Expand a desktop-entry Exec template into argv-style command arguments.
pub fn expand_exec_template(
    template: &str,
    app: &ShellApp,
    targets: &[ShellTarget],
) -> Result<Vec<String>, ExecExpansionError> {
    let tokens = split_exec_template(template)?;
    let mut expanded = Vec::new();

    for token in tokens {
        expanded.extend(expand_token(&token, app, targets)?);
    }

    Ok(expanded)
}

fn expand_token(
    token: &str,
    app: &ShellApp,
    targets: &[ShellTarget],
) -> Result<Vec<String>, ExecExpansionError> {
    if token == "%F" {
        return targets
            .iter()
            .map(|target| {
                target
                    .as_file_argument()
                    .ok_or(ExecExpansionError::RequiresFileTarget { field: 'F' })
            })
            .collect();
    }
    if token == "%U" {
        return Ok(targets.iter().map(ShellTarget::as_uri_argument).collect());
    }

    let mut output = String::new();
    let mut chars = token.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        let Some(field) = chars.next() else {
            output.push('%');
            break;
        };

        match field {
            '%' => output.push('%'),
            'f' => output.push_str(&single_file_arg(targets, 'f')?),
            'u' => output.push_str(&single_uri_arg(targets, 'u')?),
            'c' => output.push_str(&app.entry.name),
            'k' => output.push_str(&app.id),
            'F' | 'U' => return Err(ExecExpansionError::UnsupportedFieldCode(field)),
            other => return Err(ExecExpansionError::UnsupportedFieldCode(other)),
        }
    }

    Ok(vec![output])
}

fn single_file_arg(targets: &[ShellTarget], field: char) -> Result<String, ExecExpansionError> {
    let Some(target) = targets.first() else {
        return Err(ExecExpansionError::RequiresTarget { field });
    };
    target
        .as_file_argument()
        .ok_or(ExecExpansionError::RequiresFileTarget { field })
}

fn single_uri_arg(targets: &[ShellTarget], field: char) -> Result<String, ExecExpansionError> {
    let Some(target) = targets.first() else {
        return Err(ExecExpansionError::RequiresTarget { field });
    };
    Ok(target.as_uri_argument())
}

fn split_exec_template(template: &str) -> Result<Vec<String>, ExecExpansionError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = template.chars().peekable();
    let mut quote = None;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err(ExecExpansionError::UnterminatedQuote);
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}
