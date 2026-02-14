import { useEffect, useCallback } from "react";
import { useAtom, useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { favoriteTrackIdsAtom } from "../atoms/favorites";
import { authTokensAtom } from "../atoms/auth";

export function useFavorites() {
  const [favoriteTrackIds, setFavoriteTrackIds] = useAtom(favoriteTrackIdsAtom);
  const authTokens = useAtomValue(authTokensAtom);

  const loadFavoriteTrackIds = useCallback(async () => {
    if (!authTokens?.user_id) return;
    try {
      const ids = await invoke<number[]>("get_favorite_track_ids", {
        userId: authTokens.user_id,
      });
      setFavoriteTrackIds(new Set(ids));
    } catch (error: any) {
      console.error("Failed to load favorite track IDs:", error);
    }
  }, [authTokens?.user_id, setFavoriteTrackIds]);

  // Load favorite track IDs when authenticated
  useEffect(() => {
    if (authTokens?.user_id) {
      loadFavoriteTrackIds();
    }
  }, [authTokens?.user_id, loadFavoriteTrackIds]);

  const addFavoriteTrack = useCallback(
    async (trackId: number): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("add_favorite_track", {
          userId: authTokens.user_id,
          trackId,
        });
        setFavoriteTrackIds((prev: Set<number>) => new Set([...prev, trackId]));
      } catch (error: any) {
        console.error("Failed to favorite track:", error);
        throw error;
      }
    },
    [authTokens?.user_id, setFavoriteTrackIds]
  );

  const removeFavoriteTrack = useCallback(
    async (trackId: number): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("remove_favorite_track", {
          userId: authTokens.user_id,
          trackId,
        });
        setFavoriteTrackIds((prev: Set<number>) => {
          const next = new Set(prev);
          next.delete(trackId);
          return next;
        });
      } catch (error: any) {
        console.error("Failed to remove favorite track:", error);
        throw error;
      }
    },
    [authTokens?.user_id, setFavoriteTrackIds]
  );

  const isAlbumFavorited = useCallback(
    async (albumId: number): Promise<boolean> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        return await invoke<boolean>("is_album_favorited", {
          userId: authTokens.user_id,
          albumId,
        });
      } catch (error: any) {
        console.error("Failed to check album favorite status:", error);
        throw error;
      }
    },
    [authTokens?.user_id]
  );

  const addFavoriteAlbum = useCallback(
    async (albumId: number): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("add_favorite_album", {
          userId: authTokens.user_id,
          albumId,
        });
      } catch (error: any) {
        console.error("Failed to favorite album:", error);
        throw error;
      }
    },
    [authTokens?.user_id]
  );

  const removeFavoriteAlbum = useCallback(
    async (albumId: number): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("remove_favorite_album", {
          userId: authTokens.user_id,
          albumId,
        });
      } catch (error: any) {
        console.error("Failed to remove favorite album:", error);
        throw error;
      }
    },
    [authTokens?.user_id]
  );

  const addFavoritePlaylist = useCallback(
    async (playlistUuid: string): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("add_favorite_playlist", {
          userId: authTokens.user_id,
          playlistUuid,
        });
      } catch (error: any) {
        console.error("Failed to favorite playlist:", error);
        throw error;
      }
    },
    [authTokens?.user_id]
  );

  const removeFavoritePlaylist = useCallback(
    async (playlistUuid: string): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("remove_favorite_playlist", {
          userId: authTokens.user_id,
          playlistUuid,
        });
      } catch (error: any) {
        console.error("Failed to remove favorite playlist:", error);
        throw error;
      }
    },
    [authTokens?.user_id]
  );

  return {
    favoriteTrackIds,
    loadFavoriteTrackIds,
    addFavoriteTrack,
    removeFavoriteTrack,
    isAlbumFavorited,
    addFavoriteAlbum,
    removeFavoriteAlbum,
    addFavoritePlaylist,
    removeFavoritePlaylist,
  };
}
