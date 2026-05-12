//! Classical Hub backend module.
//!
//! Layout (descriptive — see `docs/classical/ARCHITECTURE.md`):
//!
//!   classical/
//!     mod.rs          ← this file: re-exports + factory
//!     types.rs        ← domain types (Work / Recording / Composer / ...)
//!     matching.rs     ← D-010 cascade matcher (ISRC + text-search)
//!     catalog.rs      ← CatalogService — orchestrates providers + cache
//!     providers/
//!       mod.rs        ← ClassicalProvider trait + MbRateLimiter
//!       musicbrainz.rs ← MB browse/work/recording/artist
//!       tidal.rs      ← ISRC bridge + canonical search wrapper
//!       wikipedia.rs  ← REST summary
//!
//! Bit-perfect contract is **read-only** with respect to this module.
//! The Catalog never opens the audio device, never touches volume, never
//! routes through the writer. Playback continues to flow through the
//! existing `commands::playback` and `audio.rs` pipeline unchanged.

pub mod buckets;
pub mod catalog;
pub mod editorial;
pub mod listening_guide;
pub mod matching;
pub mod movement;
pub mod providers;
pub mod quality;
pub mod search;
pub mod types;

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::cache::DiskCache;
use crate::stats::StatsDb;
use crate::tidal_api::TidalClient;

pub use catalog::CatalogService;
pub use types::{
    BestAvailableQuality, CatalogueNumber, Composer, ComposerSummary, Era, Genre, LifeEvent,
    MatchConfidence, Movement, PerformerCredit, PerformerCreditWithRole, Recording,
    RelatedComposer, Work, WorkBucket, WorkSummary, WorkType,
};

/// Build a fully-wired `CatalogService` for the running app. Called once
/// from `lib.rs::AppState::new` and stored in `AppState`. The shared
/// `Arc<Mutex<TidalClient>>` is reused for every Tidal call; the HTTP
/// client comes from the same proxy-aware factory the rest of the app
/// uses. The stats DB handle is shared so the catalog can read editorial
/// overrides (D-021) without owning a separate connection.
pub fn build_catalog_service(
    cache: Arc<DiskCache>,
    http_client: reqwest::Client,
    tidal_client: Arc<Mutex<TidalClient>>,
    stats: Arc<StatsDb>,
) -> Arc<CatalogService> {
    let mb_rate = Arc::new(providers::MbRateLimiter::new());
    let mb = Arc::new(providers::musicbrainz::MusicBrainzProvider::new(
        http_client.clone(),
        Arc::clone(&mb_rate),
    ));
    let wikipedia = Arc::new(providers::wikipedia::WikipediaProvider::new(
        http_client.clone(),
        Arc::clone(&mb_rate),
    ));
    let tidal = Arc::new(providers::tidal::TidalProvider::new(tidal_client));
    let openopus = Arc::new(providers::openopus::OpenOpusProvider::new());
    let composers_extended =
        Arc::new(providers::composers_extended::ExtendedComposersProvider::new());
    let editorial = Arc::new(editorial::EditorialProvider::new());
    let wikidata = Arc::new(providers::wikidata::WikidataProvider::new(http_client));
    Arc::new(CatalogService::new(
        cache,
        mb_rate,
        mb,
        wikipedia,
        tidal,
        openopus,
        composers_extended,
        editorial,
        wikidata,
        stats,
    ))
}
