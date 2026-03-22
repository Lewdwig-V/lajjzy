use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeKind {
    GitHub,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrInfo {
    pub number: u32,
    pub title: String,
    pub state: PrState,
    pub review: ReviewStatus,
    pub head_ref: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Approved,
    ChangesRequested,
    ReviewRequired,
    Unknown,
}

/// Abstraction over forge (GitHub/GitLab/Gerrit) access.
/// Separate from `RepoBackend` — forge operations use different CLI tools.
pub trait ForgeBackend: Send + Sync {
    /// Which forge CLI is available, if any.
    fn forge_kind(&self) -> Option<ForgeKind>;

    /// Fetch PR/MR status from the forge.
    /// Returns `Ok(None)` when no forge CLI is available.
    fn fetch_status(&self) -> Result<Option<Vec<PrInfo>>>;
}
