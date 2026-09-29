//! Session state and the route guard (TODO M4 "認証画面・セッション処理").
//!
//! Server contract: `GET /api/auth/status → {configured, authenticated,
//! setup_required, user, csrf_token}`. A server without auth endpoints (404) is
//! treated as open (`AuthPhase::Disabled`).

use leptos::prelude::*;

use crate::api::{self, AuthStatus};

/// Where the session stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthPhase {
    /// Status not known yet.
    Loading,
    /// The server has no authentication (or none configured and not required).
    Disabled,
    /// First run: a password must be set at `/setup`.
    NeedsSetup,
    /// Log in at `/login`.
    NeedsLogin,
    /// Logged in.
    Authenticated { user: Option<String> },
    /// The status could not be fetched (server down); retried by the user.
    Unreachable(String),
}

impl AuthPhase {
    /// Whether the app screens may be shown.
    pub(crate) fn allows_app(&self) -> bool {
        matches!(self, AuthPhase::Disabled | AuthPhase::Authenticated { .. })
    }
}

/// Maps the status response to a phase (pure; unit tested).
pub(crate) fn phase_of(status: Option<&AuthStatus>) -> AuthPhase {
    let Some(s) = status else {
        return AuthPhase::Disabled;
    };
    if s.setup_required {
        AuthPhase::NeedsSetup
    } else if s.authenticated {
        let user = s.user.as_ref().and_then(|u| {
            u.as_str()
                .map(str::to_owned)
                .or_else(|| u.get("name").and_then(|n| n.as_str()).map(str::to_owned))
        });
        AuthPhase::Authenticated { user }
    } else if s.configured {
        AuthPhase::NeedsLogin
    } else {
        AuthPhase::Disabled
    }
}

/// Where the guard sends a user on `path` in `phase` (`None` = stay).
pub(crate) fn redirect_for(phase: &AuthPhase, path: &str) -> Option<String> {
    let on_auth_page = path == "/login" || path == "/setup";
    match phase {
        AuthPhase::Loading | AuthPhase::Unreachable(_) => None,
        AuthPhase::NeedsSetup => (path != "/setup").then(|| "/setup".to_owned()),
        AuthPhase::NeedsLogin => (!on_auth_page || path == "/setup").then(|| {
            if on_auth_page || path == "/" {
                "/login".to_owned()
            } else {
                format!("/login?next={}", api::enc(path))
            }
        }),
        AuthPhase::Disabled | AuthPhase::Authenticated { .. } => {
            on_auth_page.then(|| "/".to_owned())
        }
    }
}

/// Safe post-login target from `?next=` (same-origin paths only).
pub(crate) fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => n.to_owned(),
        _ => "/".to_owned(),
    }
}

/// Session context.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Auth {
    pub(crate) phase: RwSignal<AuthPhase>,
    /// Whether the server has auth endpoints at all (logout button).
    pub(crate) enabled: RwSignal<bool>,
}

impl Auth {
    pub(crate) fn new() -> Self {
        Self {
            phase: RwSignal::new(AuthPhase::Loading),
            enabled: RwSignal::new(false),
        }
    }

    /// Re-reads `/api/auth/status` and updates the phase and CSRF token.
    pub(crate) async fn refresh(self) {
        match api::auth_status().await {
            Ok(status) => {
                self.enabled.set(status.is_some());
                api::set_csrf(status.as_ref().and_then(|s| s.csrf_token.clone()));
                self.phase.set(phase_of(status.as_ref()));
            }
            Err(e) => self.phase.set(AuthPhase::Unreachable(e.code().to_owned())),
        }
    }

    /// Spawns [`Auth::refresh`].
    pub(crate) fn refresh_later(self) {
        leptos::task::spawn_local(self.refresh());
    }

    /// Logs out and re-checks the status (the guard then shows `/login`).
    pub(crate) async fn logout(self) -> Result<(), api::ApiError> {
        let r = api::auth_logout().await;
        api::set_csrf(None);
        self.refresh().await;
        r
    }
}

/// The [`Auth`] context.
pub(crate) fn use_auth() -> Auth {
    expect_context::<Auth>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phases() {
        assert_eq!(phase_of(None), AuthPhase::Disabled);
        let s = |c, a, r| AuthStatus {
            configured: c,
            authenticated: a,
            setup_required: r,
            user: Some(json!("admin")),
            csrf_token: None,
        };
        assert_eq!(
            phase_of(Some(&s(false, false, true))),
            AuthPhase::NeedsSetup
        );
        assert_eq!(
            phase_of(Some(&s(true, false, false))),
            AuthPhase::NeedsLogin
        );
        assert_eq!(
            phase_of(Some(&s(true, true, false))),
            AuthPhase::Authenticated {
                user: Some("admin".into())
            }
        );
        assert_eq!(phase_of(Some(&s(false, false, false))), AuthPhase::Disabled);
        let mut obj = s(true, true, false);
        obj.user = Some(json!({"name": "ops"}));
        assert_eq!(
            phase_of(Some(&obj)),
            AuthPhase::Authenticated {
                user: Some("ops".into())
            }
        );
    }

    #[test]
    fn redirects() {
        let login = AuthPhase::NeedsLogin;
        assert_eq!(
            redirect_for(&login, "/jobs").as_deref(),
            Some("/login?next=%2Fjobs")
        );
        assert_eq!(redirect_for(&login, "/").as_deref(), Some("/login"));
        assert_eq!(redirect_for(&login, "/login"), None);
        assert_eq!(redirect_for(&login, "/setup").as_deref(), Some("/login"));
        assert_eq!(
            redirect_for(&AuthPhase::NeedsSetup, "/jobs").as_deref(),
            Some("/setup")
        );
        assert_eq!(redirect_for(&AuthPhase::NeedsSetup, "/setup"), None);
        let ok = AuthPhase::Authenticated { user: None };
        assert_eq!(redirect_for(&ok, "/login").as_deref(), Some("/"));
        assert_eq!(redirect_for(&ok, "/jobs"), None);
        assert_eq!(redirect_for(&AuthPhase::Loading, "/jobs"), None);
        assert!(ok.allows_app() && AuthPhase::Disabled.allows_app() && !login.allows_app());
    }

    #[test]
    fn next_targets() {
        assert_eq!(safe_next(Some("/jobs")), "/jobs");
        assert_eq!(safe_next(Some("//evil.example")), "/");
        assert_eq!(safe_next(Some("https://evil.example")), "/");
        assert_eq!(safe_next(Some("/\\evil")), "/");
        assert_eq!(safe_next(None), "/");
    }
}
