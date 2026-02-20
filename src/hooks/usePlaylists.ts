import { useCallback } from "react";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { userPlaylistsAtom, favoritePlaylistsAtom, deletedPlaylistIdsAtom } from "../atoms/playlists";
import { authTokensAtom } from "../atoms/auth";
import { invalidateCache } from "../api/tidal";
import type { Playlist } from "../types";

export function usePlaylists() {
  const [userPlaylists, setUserPlaylists] = useAtom(userPlaylistsAtom);
  const favoritePlaylists = useAtomValue(favoritePlaylistsAtom);
  const setFavoritePlaylists = useSetAtom(favoritePlaylistsAtom);
  const setDeletedPlaylistIds = useSetAtom(deletedPlaylistIdsAtom);
  const authTokens = useAtomValue(authTokensAtom);

  const createPlaylist = useCallback(
    async (title: string, description: string = ""): Promise<Playlist> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        const playlist = await invoke<Playlist>("create_playlist", {
          userId: authTokens.user_id,
          title,
          description,
        });
        setUserPlaylists((prev) => [playlist, ...prev]);
        return playlist;
      } catch (error: any) {
        console.error("Failed to create playlist:", error);
        throw error;
      }
    },
    [authTokens?.user_id, setUserPlaylists]
  );

  const addTrackToPlaylist = useCallback(
    async (playlistId: string, trackId: number): Promise<void> => {
      try {
        await invoke("add_track_to_playlist", {
          playlistId,
          trackId: trackId,
        });
        invalidateCache(`playlist:${playlistId}`);
        invalidateCache(`playlist-page:${playlistId}`);
      } catch (error: any) {
        console.error("Failed to add track to playlist:", error);
        throw error;
      }
    },
    []
  );

  const removeTrackFromPlaylist = useCallback(
    async (playlistId: string, index: number): Promise<void> => {
      try {
        await invoke("remove_track_from_playlist", {
          playlistId,
          index,
        });
        invalidateCache(`playlist:${playlistId}`);
        invalidateCache(`playlist-page:${playlistId}`);
      } catch (error: any) {
        console.error("Failed to remove track from playlist:", error);
        throw error;
      }
    },
    []
  );

  const deletePlaylist = useCallback(
    async (playlistId: string): Promise<void> => {
      if (!authTokens?.user_id) throw new Error("Not authenticated");
      try {
        await invoke("delete_playlist", {
          userId: authTokens.user_id,
          playlistId,
        });
        setUserPlaylists((prev) => prev.filter((p) => p.uuid !== playlistId));
        setFavoritePlaylists((prev) => prev.filter((p) => p.uuid !== playlistId));
        setDeletedPlaylistIds((prev: Set<string>) => new Set(prev).add(playlistId));
        invalidateCache(`playlist:${playlistId}`);
        invalidateCache(`playlist-page:${playlistId}`);
        invalidateCache("user-playlists");
        invalidateCache("fav-playlists");
      } catch (error: any) {
        console.error("Failed to delete playlist:", error);
        throw error;
      }
    },
    [authTokens?.user_id, setUserPlaylists, setFavoritePlaylists, setDeletedPlaylistIds]
  );

  const addTracksToPlaylist = useCallback(
    async (playlistId: string, trackIds: number[]): Promise<void> => {
      try {
        await invoke("add_tracks_to_playlist", {
          playlistId,
          trackIds,
        });
        invalidateCache(`playlist:${playlistId}`);
        invalidateCache(`playlist-page:${playlistId}`);
      } catch (error: any) {
        console.error("Failed to add tracks to playlist:", error);
        throw error;
      }
    },
    []
  );

  return {
    userPlaylists,
    favoritePlaylists,
    createPlaylist,
    deletePlaylist,
    addTrackToPlaylist,
    removeTrackFromPlaylist,
    addTracksToPlaylist,
  };
}
