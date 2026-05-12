import { useCallback, startTransition } from "react";
import { useAtom } from "jotai";
import { currentViewAtom } from "../atoms/navigation";
import type { AppView } from "../types";

function navigate(setCurrentView: (view: AppView) => void, view: AppView) {
  window.history.pushState(view, "");
  // Wrap in startTransition so React can show the new page's skeleton
  // immediately without blocking on unmounting the old page's heavy DOM.
  startTransition(() => {
    setCurrentView(view);
  });
}

export function useNavigation() {
  const [currentView, setCurrentView] = useAtom(currentViewAtom);

  // NOTE: Popstate listener has been moved to AppInitializer
  // to avoid registering once per component that calls useNavigation().

  const navigateToAlbum = useCallback(
    (
      albumId: number,
      albumInfo?: { title: string; cover?: string; artistName?: string },
    ) => {
      navigate(setCurrentView, { type: "album", albumId, albumInfo });
    },
    [setCurrentView],
  );

  const navigateToPlaylist = useCallback(
    (
      playlistId: string,
      playlistInfo?: {
        title: string;
        image?: string;
        description?: string;
        creatorName?: string;
        numberOfTracks?: number;
        isUserPlaylist?: boolean;
      },
    ) => {
      navigate(setCurrentView, { type: "playlist", playlistId, playlistInfo });
    },
    [setCurrentView],
  );

  const navigateToFavorites = useCallback(() => {
    navigate(setCurrentView, { type: "favorites" });
  }, [setCurrentView]);

  const navigateHome = useCallback(() => {
    navigate(setCurrentView, { type: "home" });
  }, [setCurrentView]);

  const navigateToSearch = useCallback(
    (query: string) => {
      navigate(setCurrentView, { type: "search", query });
    },
    [setCurrentView],
  );

  const navigateToViewAll = useCallback(
    (title: string, apiPath: string, artistId?: number) => {
      navigate(setCurrentView, { type: "viewAll", title, apiPath, artistId });
    },
    [setCurrentView],
  );

  const navigateToArtist = useCallback(
    (artistId: number, artistInfo?: { name: string; picture?: string }) => {
      navigate(setCurrentView, { type: "artist", artistId, artistInfo });
    },
    [setCurrentView],
  );

  const navigateToMix = useCallback(
    (
      mixId: string,
      mixInfo?: { title: string; image?: string; subtitle?: string; mixType?: string },
    ) => {
      navigate(setCurrentView, { type: "mix", mixId, mixInfo });
    },
    [setCurrentView],
  );

  const navigateToArtistTracks = useCallback(
    (artistId: number, artistName: string) => {
      navigate(setCurrentView, { type: "artistTracks", artistId, artistName });
    },
    [setCurrentView],
  );

  const navigateToExplore = useCallback(() => {
    navigate(setCurrentView, { type: "explore" });
  }, [setCurrentView]);

  const navigateToStats = useCallback(() => {
    navigate(setCurrentView, { type: "stats" });
  }, [setCurrentView]);

  const navigateToExplorePage = useCallback(
    (apiPath: string, title: string) => {
      navigate(setCurrentView, { type: "explorePage", apiPath, title });
    },
    [setCurrentView],
  );

  /** Phase 1 (Classical Hub): navigate to the Work page for a given
   *  parent-work MBID. Reuses the `explorePage` view shape with a
   *  reserved `classical://work/...` prefix so we don't add a new
   *  variant to the AppView union — keeps `App.tsx` switch additive. */
  const navigateToClassicalWork = useCallback(
    (workMbid: string, title?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://work/${workMbid}`,
        title: title ?? "Classical work",
      });
    },
    [setCurrentView],
  );

  /** Phase 2: enter the Classical Hub landing page from anywhere. Uses
   *  the same `classical://` prefix family as Phase 1; routing is
   *  branched in `App.tsx::renderView`. */
  const navigateToClassicalHub = useCallback(() => {
    navigate(setCurrentView, {
      type: "explorePage",
      apiPath: "classical://hub",
      title: "Classical Hub",
    });
  }, [setCurrentView]);

  /** Phase 2: navigate to a Composer page. */
  const navigateToClassicalComposer = useCallback(
    (composerMbid: string, name?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://composer/${composerMbid}`,
        title: name ?? "Composer",
      });
    },
    [setCurrentView],
  );

  /** Phase 9 (D-043): navigate to a specific tab within a ComposerPage.
   *  `tab` is encoded as a query param so back-nav restores it. */
  const navigateToClassicalComposerTab = useCallback(
    (composerMbid: string, tab: "about" | "works" | "albums" | "popular", name?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://composer/${composerMbid}?tab=${tab}`,
        title: name ?? "Composer",
      });
    },
    [setCurrentView],
  );

  /** Phase 9 (F9.3 / D-040): drill-down into a single bucket of a
   *  composer's catalogue. The route is
   *  `classical://composer/{mbid}/bucket/{bucket}`. */
  const navigateToClassicalBucket = useCallback(
    (composerMbid: string, bucket: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://composer/${composerMbid}/bucket/${bucket}`,
        title: bucket,
      });
    },
    [setCurrentView],
  );

  /** Phase 2: navigate to a browse axis page. `axis` is one of
   *  "composers" | "periods" | "genres". */
  const navigateToClassicalBrowse = useCallback(
    (axis: "composers" | "periods" | "genres") => {
      const titles: Record<string, string> = {
        composers: "Browse composers",
        periods: "Browse periods",
        genres: "Browse genres",
      };
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://browse/${axis}`,
        title: titles[axis] ?? "Browse",
      });
    },
    [setCurrentView],
  );

  /** Phase 5 (F5.1): open the Classical search page, optionally
   *  pre-populated with a query. */
  const navigateToClassicalSearch = useCallback(
    (initialQuery?: string) => {
      const suffix = initialQuery
        ? `?q=${encodeURIComponent(initialQuery)}`
        : "";
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://search${suffix}`,
        title: "Classical search",
      });
    },
    [setCurrentView],
  );

  /** Phase 2: era-filtered composer list (BrowsePeriods drill-down). */
  const navigateToClassicalEra = useCallback(
    (era: string, label?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://era/${era}`,
        title: label ?? era,
      });
    },
    [setCurrentView],
  );

  /** Phase 6 (D-022): browse-by-conductor / orchestra discography. */
  const navigateToClassicalArtist = useCallback(
    (artistMbid: string, name?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://artist/${artistMbid}`,
        title: name ?? "Artist",
      });
    },
    [setCurrentView],
  );

  /** Phase 6 (F6.10): recording comparison view for a single work. */
  const navigateToClassicalCompare = useCallback(
    (workMbid: string, title?: string) => {
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: `classical://compare/${workMbid}`,
        title: title ?? "Recording comparison",
      });
    },
    [setCurrentView],
  );

  /** Phase 6 (F6.3): library facets within the Hub. */
  const navigateToClassicalLibrary = useCallback(
    (facet?: "works" | "recordings" | "composers" | "performers") => {
      const path = facet
        ? `classical://library/${facet}`
        : "classical://library";
      navigate(setCurrentView, {
        type: "explorePage",
        apiPath: path,
        title: "Classical library",
      });
    },
    [setCurrentView],
  );

  const navigateToLibraryViewAll = useCallback(
    (libraryType: "playlists" | "albums" | "artists" | "mixes") => {
      navigate(setCurrentView, { type: "libraryViewAll", libraryType });
    },
    [setCurrentView],
  );

  const navigateToPlaylistFolder = useCallback(
    (folderId: string, folderName: string) => {
      navigate(setCurrentView, {
        type: "libraryViewAll",
        libraryType: "playlists",
        folderId,
        folderName,
      });
    },
    [setCurrentView],
  );

  return {
    currentView,
    navigateToAlbum,
    navigateToPlaylist,
    navigateToFavorites,
    navigateHome,
    navigateToSearch,
    navigateToViewAll,
    navigateToArtist,
    navigateToArtistTracks,
    navigateToMix,
    navigateToExplore,
    navigateToExplorePage,
    navigateToClassicalWork,
    navigateToClassicalHub,
    navigateToClassicalComposer,
    navigateToClassicalComposerTab,
    navigateToClassicalBucket,
    navigateToClassicalBrowse,
    navigateToClassicalSearch,
    navigateToClassicalEra,
    navigateToClassicalArtist,
    navigateToClassicalCompare,
    navigateToClassicalLibrary,
    navigateToLibraryViewAll,
    navigateToPlaylistFolder,
    navigateToStats,
  };
}
