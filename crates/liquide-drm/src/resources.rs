//! Aggregated DRM topology snapshot for one-shot enumeration.

use crate::connector::{ConnectorId, ConnectorInfo, enumerate_connectors};
use crate::crtc::{CrtcId, CrtcInfo, enumerate_crtcs};
use crate::device::DrmDevice;
use crate::encoder::{EncoderId, EncoderInfo, enumerate_encoders};
use crate::error::Result;
use crate::plane::{PlaneId, PlaneInfo, enumerate_planes};

/// One-shot snapshot of the DRM device's topology: all connectors,
/// encoders, CRTCs, and planes captured at gather time.
#[derive(Debug, Clone)]
pub struct DrmResources {
    pub connectors: Vec<ConnectorInfo>,
    pub encoders: Vec<EncoderInfo>,
    pub crtcs: Vec<CrtcInfo>,
    pub planes: Vec<PlaneInfo>,
}

impl DrmResources {
    /// Gathers a fresh snapshot from `device` by invoking the four
    /// per-resource-type enumerators in turn.
    ///
    /// On non-Linux hosts, every enumerator returns `Ok(Vec::new())`,
    /// so this returns an empty snapshot.
    pub fn gather(device: &DrmDevice) -> Result<Self> {
        let connectors = enumerate_connectors(device)?;
        let encoders = enumerate_encoders(device)?;
        let crtcs = enumerate_crtcs(device)?;
        let planes = enumerate_planes(device)?;
        Ok(Self { connectors, encoders, crtcs, planes })
    }

    /// Convenience: find a connector by id.
    pub fn connector(&self, id: ConnectorId) -> Option<&ConnectorInfo> {
        self.connectors.iter().find(|c| c.id == id)
    }

    /// Convenience: find an encoder by id.
    pub fn encoder(&self, id: EncoderId) -> Option<&EncoderInfo> {
        self.encoders.iter().find(|e| e.id == id)
    }

    /// Convenience: find a CRTC by id.
    pub fn crtc(&self, id: CrtcId) -> Option<&CrtcInfo> {
        self.crtcs.iter().find(|c| c.id == id)
    }

    /// Convenience: find a plane by id.
    pub fn plane(&self, id: PlaneId) -> Option<&PlaneInfo> {
        self.planes.iter().find(|p| p.id == id)
    }
}
