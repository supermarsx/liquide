//! Authorization-gated network manager (the manager / dispatch layer).
//!
//! `liquide-network` has no dispatch wrapper of its own: each platform's
//! [`NetworkManager`] *is* a direct [`NetworkBackend`] implementation, and the
//! primary Linux impl is out-of-lock for t51-e5. So rather than gate inside the
//! platform impls (which would either be impossible to reach cleanly or leave
//! the Linux path ungated — a fail-open hole), this module introduces the
//! manager/dispatch layer the wiring plan assumed: a thin wrapper that owns an
//! [`AuthorizationRuntime`] plus a calling [`Subject`], gates each destructive
//! mutation through the canonical facade, and only then delegates to the inner
//! backend.
//!
//! Per USER-CONFIRMED CHECKPOINT A, the gated destructive/system-state ops are:
//! `connect_wifi`, `forget_wifi`, `connect_vpn`, and `set_airplane_mode`.
//! `enable_interface` / `disable_interface` are treated as cosmetic/per-session
//! and are NOT gated (delegated straight through). All read-only queries
//! (`list_interfaces`, `get_access_points`, `check_connectivity`, ...) and the
//! recoverable `disconnect_*` / `scan_wifi` ops also pass through ungated.
//!
//! Enforcement is **fail-closed**: if [`AuthorizationRuntime::authorize`]
//! returns anything other than `Granted`, the mutation is refused with
//! [`NetworkError::PermissionDenied`] and the inner backend is never called.

use liquide_authz_runtime::{AuthorizationRuntime, Subject};

use crate::{NetworkBackend, NetworkError};

/// Catalog keys for the gated network mutations (Checkpoint A).
mod action {
    pub const CONNECT_WIFI: &str = "network.connect_wifi";
    pub const FORGET_WIFI: &str = "network.forget_wifi";
    pub const CONNECT_VPN: &str = "network.connect_vpn";
    pub const SET_AIRPLANE_MODE: &str = "network.set_airplane_mode";
}

/// A [`NetworkBackend`] wrapper that gates destructive mutations through the
/// canonical authorization facade before delegating to an inner backend.
///
/// Construct one with [`GatedNetworkManager::new`], handing it the platform
/// [`NetworkManager`](crate::NetworkManager) (or any backend / test stub), the
/// shared [`AuthorizationRuntime`], and the [`Subject`] making the request.
pub struct GatedNetworkManager<B: NetworkBackend> {
    backend: B,
    runtime: AuthorizationRuntime,
    subject: Subject,
}

impl<B: NetworkBackend> GatedNetworkManager<B> {
    /// Wrap `backend`, gating its destructive ops for `subject` through
    /// `runtime`.
    pub fn new(backend: B, runtime: AuthorizationRuntime, subject: Subject) -> Self {
        Self {
            backend,
            runtime,
            subject,
        }
    }

    /// Immutable access to the wrapped backend (read-only inspection).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Mutable access to the wrapped backend.
    ///
    /// Use this for the read-only queries and ungated/per-session ops the
    /// manager does not gate (e.g. `list_interfaces`, `scan_wifi`,
    /// `disconnect_wifi`, `enable_interface`); only the four Checkpoint-A
    /// destructive mutations are intercepted by this manager.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Run the authorization check for `action_id`. Fail-closed: any result
    /// other than `Granted` becomes a [`NetworkError::PermissionDenied`] and
    /// the caller must NOT touch the backend.
    fn gate(&mut self, action_id: &str) -> Result<(), NetworkError> {
        let result = self.runtime.authorize(action_id, &self.subject, None);
        if result.is_granted() {
            Ok(())
        } else {
            Err(NetworkError::PermissionDenied)
        }
    }

    // ── Gated destructive / system-state mutations (Checkpoint A) ───────
    //
    // Inherent methods, not a `NetworkBackend` impl: `AuthorizationAgent`
    // holds a `Box<dyn CredentialVerifier>` that is not `Send`, while
    // `NetworkBackend: Send`. The manager therefore *wraps* a backend and
    // gates these four ops; it does not itself implement the backend trait.

    /// Gated `connect_wifi`. Denied authorization blocks the call: the inner
    /// backend is never reached and [`NetworkError::PermissionDenied`] returns.
    pub fn connect_wifi(&mut self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
        self.gate(action::CONNECT_WIFI)?;
        self.backend.connect_wifi(ssid, password)
    }

    /// Gated `forget_wifi`. Denied authorization blocks the backend call.
    pub fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        self.gate(action::FORGET_WIFI)?;
        self.backend.forget_wifi(ssid)
    }

    /// Gated `connect_vpn`. Denied authorization blocks the backend call.
    pub fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        self.gate(action::CONNECT_VPN)?;
        self.backend.connect_vpn(id)
    }

    /// Gated `set_airplane_mode`. Denied authorization blocks the backend call.
    pub fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        self.gate(action::SET_AIRPLANE_MODE)?;
        self.backend.set_airplane_mode(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccessPoint, ConnectivityState, InterfaceId, NetworkEvent, NetworkInterface, VpnConnection,
    };
    use liquide_authz_runtime::{ActionCatalog, AuthorizationRuntime, InMemoryAuditSink};

    /// A spying backend that records which mutations actually reached it and
    /// returns success, so a test can distinguish "blocked before backend"
    /// from "backend ran".
    #[derive(Default)]
    struct SpyBackend {
        connect_wifi_calls: u32,
        forget_wifi_calls: u32,
        connect_vpn_calls: u32,
        set_airplane_calls: u32,
        enable_iface_calls: u32,
    }

    impl NetworkBackend for SpyBackend {
        fn list_interfaces(&self) -> Vec<NetworkInterface> {
            Vec::new()
        }
        fn get_interface(&self, _id: &InterfaceId) -> Option<NetworkInterface> {
            None
        }
        fn scan_wifi(&mut self) -> Result<(), NetworkError> {
            Ok(())
        }
        fn get_access_points(&self) -> Vec<AccessPoint> {
            Vec::new()
        }
        fn connect_wifi(
            &mut self,
            _ssid: &str,
            _password: Option<&str>,
        ) -> Result<(), NetworkError> {
            self.connect_wifi_calls += 1;
            Ok(())
        }
        fn disconnect_wifi(&mut self, _interface_id: &InterfaceId) -> Result<(), NetworkError> {
            Ok(())
        }
        fn forget_wifi(&mut self, _ssid: &str) -> Result<(), NetworkError> {
            self.forget_wifi_calls += 1;
            Ok(())
        }
        fn enable_interface(&mut self, _id: &InterfaceId) -> Result<(), NetworkError> {
            self.enable_iface_calls += 1;
            Ok(())
        }
        fn disable_interface(&mut self, _id: &InterfaceId) -> Result<(), NetworkError> {
            Ok(())
        }
        fn list_vpn_connections(&self) -> Vec<VpnConnection> {
            Vec::new()
        }
        fn connect_vpn(&mut self, _id: &str) -> Result<(), NetworkError> {
            self.connect_vpn_calls += 1;
            Ok(())
        }
        fn disconnect_vpn(&mut self, _id: &str) -> Result<(), NetworkError> {
            Ok(())
        }
        fn check_connectivity(&self) -> ConnectivityState {
            ConnectivityState::None
        }
        fn is_airplane_mode(&self) -> bool {
            false
        }
        fn set_airplane_mode(&mut self, _enabled: bool) -> Result<(), NetworkError> {
            self.set_airplane_calls += 1;
            Ok(())
        }
        fn poll_events(&mut self) -> Vec<NetworkEvent> {
            Vec::new()
        }
    }

    fn subject() -> Subject {
        Subject::new(1000, 4242, "session-test")
    }

    /// Build a runtime whose default catalog has been forced so the four
    /// Checkpoint-A network ops are gated (so the deny path is exercised even
    /// where e1's default leaves a key ungated, e.g. set_airplane_mode).
    fn gated_runtime() -> AuthorizationRuntime {
        let mut catalog = ActionCatalog::with_defaults();
        for key in [
            action::CONNECT_WIFI,
            action::FORGET_WIFI,
            action::CONNECT_VPN,
            action::SET_AIRPLANE_MODE,
        ] {
            catalog.set_gated(key, true);
        }
        AuthorizationRuntime::new("test-user", catalog, Box::new(InMemoryAuditSink::new()))
    }

    /// In this Windows host test env no credential verifier is available, so
    /// every gated (AdminPassword/UserPassword) op resolves to a denial — the
    /// fail-closed negative path. Assert NO backend call happened.
    #[test]
    fn denied_authorization_blocks_connect_wifi_no_backend_call() {
        let backend = SpyBackend::default();
        let mut mgr = GatedNetworkManager::new(backend, gated_runtime(), subject());

        let result = mgr.connect_wifi("HomeWiFi", Some("hunter2"));

        assert!(matches!(result, Err(NetworkError::PermissionDenied)));
        assert_eq!(
            mgr.backend().connect_wifi_calls,
            0,
            "backend must not be called when authorization is denied"
        );
    }

    #[test]
    fn denied_authorization_blocks_forget_wifi_no_backend_call() {
        let backend = SpyBackend::default();
        let mut mgr = GatedNetworkManager::new(backend, gated_runtime(), subject());

        let result = mgr.forget_wifi("HomeWiFi");

        assert!(matches!(result, Err(NetworkError::PermissionDenied)));
        assert_eq!(mgr.backend().forget_wifi_calls, 0);
    }

    #[test]
    fn denied_authorization_blocks_connect_vpn_no_backend_call() {
        let backend = SpyBackend::default();
        let mut mgr = GatedNetworkManager::new(backend, gated_runtime(), subject());

        let result = mgr.connect_vpn("work-vpn");

        assert!(matches!(result, Err(NetworkError::PermissionDenied)));
        assert_eq!(mgr.backend().connect_vpn_calls, 0);
    }

    #[test]
    fn denied_authorization_blocks_set_airplane_mode_no_backend_call() {
        let backend = SpyBackend::default();
        let mut mgr = GatedNetworkManager::new(backend, gated_runtime(), subject());

        let result = mgr.set_airplane_mode(true);

        assert!(matches!(result, Err(NetworkError::PermissionDenied)));
        assert_eq!(mgr.backend().set_airplane_calls, 0);
    }

    /// Granted authorization proceeds to the backend. We drive the grant path
    /// through an *ungated* catalog entry (NoAuth would require none here; the
    /// facade returns Granted for ungated keys without a verifier), which is
    /// the seam's "allow" branch.
    #[test]
    fn granted_authorization_proceeds_to_backend() {
        // Leave the four keys at their e1 defaults but force them ungated so
        // the facade returns Granted (no credential verifier on this host).
        let mut catalog = ActionCatalog::with_defaults();
        for key in [
            action::CONNECT_WIFI,
            action::FORGET_WIFI,
            action::CONNECT_VPN,
            action::SET_AIRPLANE_MODE,
        ] {
            catalog.set_gated(key, false);
        }
        let runtime =
            AuthorizationRuntime::new("test-user", catalog, Box::new(InMemoryAuditSink::new()));
        let mut mgr = GatedNetworkManager::new(SpyBackend::default(), runtime, subject());

        assert!(mgr.connect_wifi("HomeWiFi", Some("pw")).is_ok());
        assert!(mgr.forget_wifi("HomeWiFi").is_ok());
        assert!(mgr.connect_vpn("work-vpn").is_ok());
        assert!(mgr.set_airplane_mode(true).is_ok());

        assert_eq!(mgr.backend().connect_wifi_calls, 1);
        assert_eq!(mgr.backend().forget_wifi_calls, 1);
        assert_eq!(mgr.backend().connect_vpn_calls, 1);
        assert_eq!(mgr.backend().set_airplane_calls, 1);
    }

    /// Ungated ops (enable_interface) are never gated regardless of auth state:
    /// they reach the backend even when the runtime would deny gated ops. The
    /// manager does not intercept them — callers reach them via `backend_mut`.
    #[test]
    fn ungated_enable_interface_is_never_blocked() {
        let mut mgr = GatedNetworkManager::new(SpyBackend::default(), gated_runtime(), subject());
        let id = InterfaceId("eth0".to_string());

        assert!(mgr.backend_mut().enable_interface(&id).is_ok());
        assert_eq!(mgr.backend().enable_iface_calls, 1);
    }
}
