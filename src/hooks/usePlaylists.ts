import { useCallback } from "react";
import { useAtom, useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { userPlaylistsAtom, favoritePlaylistsAtom } from "../atoms/playlists";
import { authTokensAtom } from "../atoms/auth";
import type { Playlist } from "../types";

export function usePlaylists() {
  const [userPlaylists, setUserPlaylists] = useAtom(userPlaylistsAtom);
  const favoritePlaylists = useAtomValue(favoritePlaylistsAtom);
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
      } catch (error: any) {
        console.error("Failed to remove track from playlist:", error);
        throw error;
      }
    },
    []
  );

  const addTracksToPlaylist = useCallback(
    async (playlistId: string, trackIds: number[]): Promise<void> => {
      try {
        await invoke("add_tracks_to_playlist", {
          playlistId,
          trackIds,
        });
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
    addTrackToPlaylist,
    removeTrackFromPlaylist,
    addTracksToPlaylist,
  };
}
