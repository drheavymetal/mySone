//! Provider trait and shared HTTP/rate-limit primitives for the Classical
//! Hub catalog. Each provider implements `enrich_*` best-effort: a
//! provider that fails or has no data for a given entity returns `Ok(())`
//! without mutating it, and the next provider in the chain takes over.
//!
//! Reference: CLASSICAL_DESIGN.md §5.2.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::SoneError;

use super::types::{Composer, Recording, Work};

pub mod composers_extended;
pub mod musicbrainz;
pub mod openopus;
pub mod tidal;
pub mod wikidata;
pub mod wikipedia;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ClassicalProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Best-effort enrichment of a Composer object. The provider fills
    /// the fields it owns and leaves the rest untouched.
    async fn enrich_composer(&self, _c: &mut Composer) -> Result<(), SoneError> {
        Ok(())
    }

    /// Best-effort enrichment of a Work object.
    async fn enrich_work(&self, _w: &mut Work) -> Result<(), SoneError> {
        Ok(())
    }

    /// Best-effort enrichment of a single Recording.
    async fn enrich_recording(&self, _r: &mut Recording) -> Result<(), SoneError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MusicBrainz rate limiter shared by every provider that hits MB
// ---------------------------------------------------------------------------

/// MusicBrainz mandates ≤1 req/s per client. We use 1.1 s to leave a
/// 100 ms cushion against clock skew between caller and MB's edge.
pub const MB_MIN_INTERVAL: Duration = Duration::from_millis(1100);

/// Single shared rate-limiter so multiple consumers (MB provider, the
/// Wikipedia composer pre-warm, future Wikidata SPARQL) cannot
/// inadvertently double the request rate.
pub struct MbRateLimiter {
    last: Mutex<Instant>,
}

impl MbRateLimiter {
    pub fn new() -> Self {
        // Initialise so the very first call doesn't block.
        Self {
            last: Mutex::new(Instant::now() - MB_MIN_INTERVAL),
        }
    }

    /// Wait until the next request slot is available, then mark it taken.
    pub async fn acquire(&self) {
        let mut last = self.last.lock().await;
        let elapsed = last.elapsed();
        if elapsed < MB_MIN_INTERVAL {
            tokio::time::sleep(MB_MIN_INTERVAL - elapsed).await;
        }
        *last = Instant::now();
    }
}

impl Default for MbRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper: bundle providers as `Arc<dyn ClassicalProvider>` so the Catalog
// service can fan out without owning concrete types.
// ---------------------------------------------------------------------------

pub type SharedProvider = Arc<dyn ClassicalProvider>;
