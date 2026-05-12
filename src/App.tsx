import { useEffect, useState } from "react";
import Layout from "./components/Layout";
import Home from "./components/Home";
import AlbumView from "./components/AlbumView";
import PlaylistView from "./components/PlaylistView";
import FavoritesView from "./components/FavoritesView";
import SearchView from "./components/SearchView";
import ViewAllPage from "./components/ViewAllPage";
import ArtistPage from "./components/ArtistPage";
import ArtistTracksPage from "./components/ArtistTracksPage";
import MixPage from "./components/MixPage";
import ExplorePage from "./components/ExplorePage";
import ExploreSubPage from "./components/ExploreSubPage";
import WorkPage from "./components/classical/WorkPage";
import ClassicalHubPage from "./components/classical/ClassicalHubPage";
import ComposerPage from "./components/classical/ComposerPage";
import BrowseComposers from "./components/classical/BrowseComposers";
import BrowsePeriods from "./components/classical/BrowsePeriods";
import BrowseGenres from "./components/classical/BrowseGenres";
import BrowseEra from "./components/classical/BrowseEra";
import ClassicalSearch from "./components/classical/ClassicalSearch";
import ClassicalLibrary from "./components/classical/ClassicalLibrary";
import ClassicalArtistPage from "./components/classical/ClassicalArtistPage";
import ClassicalRecordingComparison from "./components/classical/ClassicalRecordingComparison";
import BrowseComposerBucket from "./components/classical/BrowseComposerBucket";
import type { WorkBucket as ClassicalBucketSlug } from "./types/classical";
import LibraryViewAll from "./components/LibraryViewAll";
import StatsPage from "./components/StatsPage";
import Login from "./components/Login";
import { AppInitializer } from "./components/AppInitializer";
import ErrorBoundary from "./components/ErrorBoundary";
import { useAuth } from "./hooks/useAuth";
import { useNavigation } from "./hooks/useNavigation";
import { useAtomValue } from "jotai";
import { isAuthCheckingAtom } from "./atoms/auth";
import { ToastProvider } from "./contexts/ToastContext";
import { useTheme } from "./hooks/useTheme";
import "./App.css";

const ZOOM_KEY = "sone.zoom.v1";
const ZOOM_STEP = 0.1;
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2.0;

function useZoom() {
  const [zoom, setZoom] = useState(() => {
    try {
      const saved = localStorage.getItem(ZOOM_KEY);
      if (saved) {
        const val = Number(saved);
        if (!Number.isNaN(val) && val >= ZOOM_MIN && val <= ZOOM_MAX)
          return val;
      }
    } catch {}
    return 1.0;
  });

  useEffect(() => {
    document.documentElement.style.zoom = String(zoom);
    document.documentElement.style.setProperty("--zoom", String(zoom));
  }, [zoom]);

  useEffect(() => {
    try {
      localStorage.setItem(ZOOM_KEY, String(zoom));
    } catch {}
  }, [zoom]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.repeat) return;
      if (!e.ctrlKey && !e.metaKey) return;

      if (e.key === "+" || e.key === "=") {
        e.preventDefault();
        setZoom((z) =>
          Math.min(ZOOM_MAX, Math.round((z + ZOOM_STEP) * 100) / 100),
        );
      } else if (e.key === "-") {
        e.preventDefault();
        setZoom((z) =>
          Math.max(ZOOM_MIN, Math.round((z - ZOOM_STEP) * 100) / 100),
        );
      } else if (e.key === "0") {
        e.preventDefault();
        setZoom(1.0);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}

function AppContent() {
  const { isAuthenticated } = useAuth();
  const isAuthChecking = useAtomValue(isAuthCheckingAtom);
  const { currentView, navigateHome, navigateToExplore } = useNavigation();

  if (isAuthChecking) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-th-background">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-th-accent border-t-transparent" />
      </div>
    );
  }

  if (!isAuthenticated) {
    return <Login />;
  }

  const renderView = () => {
    switch (currentView.type) {
      case "album":
        return (
          <AlbumView
            key={currentView.albumId}
            albumId={currentView.albumId}
            albumInfo={currentView.albumInfo}
            onBack={navigateHome}
          />
        );
      case "playlist":
        return (
          <PlaylistView
            key={currentView.playlistId}
            playlistId={currentView.playlistId}
            playlistInfo={currentView.playlistInfo}
            onBack={navigateHome}
          />
        );
      case "favorites":
        return <FavoritesView onBack={navigateHome} />;
      case "search":
        return (
          <SearchView
            key={currentView.query}
            query={currentView.query}
            onBack={navigateHome}
          />
        );
      case "viewAll":
        return (
          <ViewAllPage
            key={currentView.apiPath}
            title={currentView.title}
            apiPath={currentView.apiPath}
            artistId={currentView.artistId}
            onBack={navigateHome}
          />
        );
      case "artist":
        return (
          <ArtistPage
            key={currentView.artistId}
            artistId={currentView.artistId}
            artistInfo={currentView.artistInfo}
            onBack={navigateHome}
          />
        );
      case "artistTracks":
        return (
          <ArtistTracksPage
            key={currentView.artistId}
            artistId={currentView.artistId}
            artistName={currentView.artistName}
          />
        );
      case "mix":
        return (
          <MixPage
            key={currentView.mixId}
            mixId={currentView.mixId}
            mixInfo={currentView.mixInfo}
            onBack={navigateHome}
          />
        );
      case "explore":
        return <ExplorePage />;
      case "explorePage": {
        // Phase 1 + 2 (Classical Hub): apiPaths prefixed with
        // `classical://*` switch to one of the Classical Hub screens in
        // place of Tidal's editorial sub-page. The branch is purely
        // additive — every existing apiPath without a `classical://`
        // prefix continues to hit `ExploreSubPage` unchanged.
        const path = currentView.apiPath;
        const classicalWorkPrefix = "classical://work/";
        const classicalComposerPrefix = "classical://composer/";
        const classicalEraPrefix = "classical://era/";
        if (path.startsWith(classicalWorkPrefix)) {
          const mbid = path.slice(classicalWorkPrefix.length);
          return (
            <WorkPage key={path} mbid={mbid} onBack={navigateToExplore} />
          );
        }
        if (path === "classical://hub") {
          return <ClassicalHubPage key={path} onBack={navigateToExplore} />;
        }
        if (path.startsWith(classicalComposerPrefix)) {
          // Phase 9 (F9.1 / F9.3 / D-043) — extended composer routes:
          //   classical://composer/{mbid}                  → tab=about
          //   classical://composer/{mbid}?tab=works        → explicit tab
          //   classical://composer/{mbid}/bucket/{bucket}  → drill-down
          const tail = path.slice(classicalComposerPrefix.length);
          // Sub-route "/bucket/<X>" lives at the top of the parse so a
          // tab query string on the parent doesn't catch it.
          const bucketMatch = tail.match(/^([^/?]+)\/bucket\/([^/?]+)$/);
          if (bucketMatch) {
            const mbid = bucketMatch[1];
            const bucket = bucketMatch[2];
            return (
              <BrowseComposerBucket
                key={path}
                composerMbid={mbid}
                bucket={bucket as ClassicalBucketSlug}
                onBack={navigateToExplore}
              />
            );
          }
          // Strip optional `?tab=` query.
          const [mbidWithMaybeQs, ...rest] = tail.split("?");
          const qs = rest.join("?");
          let initialTab: "about" | "works" | "albums" | "popular" | undefined;
          if (qs) {
            const params = new URLSearchParams(qs);
            const t = params.get("tab");
            if (t === "about" || t === "works" || t === "albums" || t === "popular") {
              initialTab = t;
            }
          }
          return (
            <ComposerPage
              key={path}
              mbid={mbidWithMaybeQs}
              initialTab={initialTab}
              onBack={navigateToExplore}
            />
          );
        }
        if (path === "classical://browse/composers") {
          return <BrowseComposers key={path} onBack={navigateToExplore} />;
        }
        if (path === "classical://browse/periods") {
          return <BrowsePeriods key={path} onBack={navigateToExplore} />;
        }
        if (path === "classical://browse/genres") {
          return <BrowseGenres key={path} onBack={navigateToExplore} />;
        }
        if (path.startsWith(classicalEraPrefix)) {
          const era = path.slice(classicalEraPrefix.length);
          return <BrowseEra key={path} era={era} onBack={navigateToExplore} />;
        }
        // Phase 5: classical search. The optional `?q=...` carries an
        // initial query (e.g. when invoked from a deep link).
        if (path.startsWith("classical://search")) {
          const q = path.includes("?q=")
            ? decodeURIComponent(path.split("?q=")[1] ?? "")
            : undefined;
          return (
            <ClassicalSearch
              key={path}
              initialQuery={q}
              onBack={navigateToExplore}
            />
          );
        }
        // Phase 6: classical library tab (works/recordings/composers/performers).
        const classicalLibraryPrefix = "classical://library";
        if (path === classicalLibraryPrefix) {
          return (
            <ClassicalLibrary key={path} onBack={navigateToExplore} />
          );
        }
        if (path.startsWith(`${classicalLibraryPrefix}/`)) {
          const facet = path.slice(classicalLibraryPrefix.length + 1) as
            | "work"
            | "recording"
            | "composer"
            | "performer";
          // Map the URL plurals back to the singular `kind` the
          // backend expects, so deep-links match the favorite kinds.
          const kindMap: Record<string, "work" | "recording" | "composer" | "performer"> = {
            works: "work",
            recordings: "recording",
            composers: "composer",
            performers: "performer",
          };
          const initial = kindMap[facet] ?? "work";
          return (
            <ClassicalLibrary
              key={path}
              initialFacet={initial}
              onBack={navigateToExplore}
            />
          );
        }
        // Phase 6 (D-022): browse-by-conductor / orchestra discography.
        const classicalArtistPrefix = "classical://artist/";
        if (path.startsWith(classicalArtistPrefix)) {
          const mbid = path.slice(classicalArtistPrefix.length);
          return (
            <ClassicalArtistPage
              key={path}
              mbid={mbid}
              displayName={currentView.title}
              onBack={navigateToExplore}
            />
          );
        }
        // Phase 6 (F6.10): recording comparison (versions of the same work).
        const classicalComparePrefix = "classical://compare/";
        if (path.startsWith(classicalComparePrefix)) {
          const mbid = path.slice(classicalComparePrefix.length);
          return (
            <ClassicalRecordingComparison
              key={path}
              workMbid={mbid}
              onBack={navigateToExplore}
            />
          );
        }
        return (
          <ExploreSubPage
            key={path}
            apiPath={path}
            title={currentView.title}
            onBack={navigateToExplore}
          />
        );
      }
      case "libraryViewAll":
        return (
          <LibraryViewAll
            key={`${currentView.libraryType}:${currentView.folderId ?? "root"}`}
            libraryType={currentView.libraryType}
            folderId={currentView.folderId}
            folderName={currentView.folderName}
          />
        );
      case "stats":
        return <StatsPage />;
      case "home":
      default:
        return <Home />;
    }
  };

  return <Layout>{renderView()}</Layout>;
}

function App() {
  useZoom();
  useTheme();

  // Disable the default browser/webview context menu globally
  useEffect(() => {
    const handler = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", handler);
    return () => document.removeEventListener("contextmenu", handler);
  }, []);

  return (
    // F8.6 — permanent ErrorBoundary at the top of the React tree.
    // Catches render crashes from any page (Explore, Classical Hub,
    // Stats, Settings, Player). Audio + Tauri state untouched.
    <ErrorBoundary>
      <ToastProvider>
        <AppInitializer />
        <AppContent />
      </ToastProvider>
    </ErrorBoundary>
  );
}

export default App;
