import { useEffect, useCallback } from "react";
import { useAtom, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { isAuthenticatedAtom, authTokensAtom, userNameAtom } from "../atoms/auth";
import { userPlaylistsAtom, favoritePlaylistsAtom } from "../atoms/playlists";
import { isPlayingAtom, currentTrackAtom, queueAtom, historyAtom } from "../atoms/playback";
import { favoriteTrackIdsAtom } from "../atoms/favorites";
import type { AuthTokens, PkceAuthParams, DeviceAuthResponse, Playlist } from "../types";

const PLAYBACK_STATE_KEY = "tide-vibe.playback-state.v1";
const VOLUME_STATE_KEY = "tide-vibe.volume.v1";

export function useAuth() {
  const [isAuthenticated, setIsAuthenticated] = useAtom(isAuthenticatedAtom);
  const [authTokens, setAuthTokens] = useAtom(authTokensAtom);
  const [userName, setUserName] = useAtom(userNameAtom);

  // Cross-domain setters for logout / auth load
  const setUserPlaylists = useSetAtom(userPlaylistsAtom);
  const setFavoritePlaylists = useSetAtom(favoritePlaylistsAtom);
  const setIsPlaying = useSetAtom(isPlayingAtom);
  const setCurrentTrack = useSetAtom(currentTrackAtom);
  const setQueue = useSetAtom(queueAtom);
  const setHistory = useSetAtom(historyAtom);
  const setFavoriteTrackIds = useSetAtom(favoriteTrackIdsAtom);

  // Load saved auth on mount
  useEffect(() => {
    const loadAuth = async () => {
      try {
        console.log("Loading saved auth...");
        const tokens = await invoke<AuthTokens | null>("load_saved_auth");
        console.log("Loaded tokens:", tokens);

        if (tokens) {
          let userId = tokens.user_id;
          if (!userId) {
            try {
              userId = await invoke<number>("get_session_user_id");
              console.log("Got user ID from session:", userId);
            } catch (e) {
              console.error("Failed to get user ID:", e);
            }
          }

          let activeTokens = { ...tokens, user_id: userId };
          setAuthTokens(activeTokens);
          setIsAuthenticated(true);

          if (userId) {
            // Fetch user name (non-blocking)
            invoke<[string, string | null]>("get_user_profile", { userId })
              .then(([name]) => {
                if (name) setUserName(name);
              })
              .catch(() => {});

            try {
              console.log("Loading playlists for user:", userId);
              const playlists = await invoke<Playlist[]>("get_user_playlists", {
                userId: userId,
              });
              console.log("Loaded playlists:", playlists?.length);
              setUserPlaylists(playlists || []);

              invoke<Playlist[]>("get_favorite_playlists", { userId })
                .then((favPlaylists) => {
                  console.log("Loaded favorite playlists:", favPlaylists?.length);
                  setFavoritePlaylists(favPlaylists || []);
                })
                .catch((err) => {
                  console.error("Failed to load favorite playlists:", err);
                  setFavoritePlaylists([]);
                });
            } catch (playlistErr: any) {
              const errStr = String(playlistErr);
              console.error("Failed to load playlists:", playlistErr);

              if (errStr.includes("401") || errStr.includes("expired")) {
                try {
                  console.log("Token expired, attempting refresh...");
                  const refreshedTokens = await invoke<AuthTokens>(
                    "refresh_tidal_auth"
                  );
                  console.log("Token refreshed successfully");

                  activeTokens = {
                    ...refreshedTokens,
                    user_id: userId ?? refreshedTokens.user_id,
                  };
                  setAuthTokens(activeTokens);

                  const playlists = await invoke<Playlist[]>(
                    "get_user_playlists",
                    { userId: userId }
                  );
                  console.log("Loaded playlists after refresh:", playlists?.length);
                  setUserPlaylists(playlists || []);

                  invoke<Playlist[]>("get_favorite_playlists", { userId })
                    .then((favPlaylists) => setFavoritePlaylists(favPlaylists || []))
                    .catch(() => setFavoritePlaylists([]));
                } catch (refreshErr) {
                  console.error("Token refresh failed:", refreshErr);
                  setIsAuthenticated(false);
                  setAuthTokens(null);
                  setUserPlaylists([]);
                  setFavoritePlaylists([]);
                }
              } else {
                setUserPlaylists([]);
              }
            }
          }
        }
      } catch (err) {
        console.error("Failed to load saved auth:", err);
      }
    };

    loadAuth();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const importSession = useCallback(
    async (
      clientId: string,
      clientSecret: string,
      refreshToken: string,
      accessToken?: string
    ): Promise<AuthTokens> => {
      try {
        const tokens = await invoke<AuthTokens>("import_session", {
          clientId,
          clientSecret,
          refreshToken,
          accessToken: accessToken || null,
        });
        let userId = tokens.user_id;
        if (!userId) {
          try {
            userId = await invoke<number>("get_session_user_id");
          } catch (e) {
            console.error("Failed to get user ID:", e);
          }
        }
        const updatedTokens = { ...tokens, user_id: userId };
        setAuthTokens(updatedTokens);
        setIsAuthenticated(true);
        return updatedTokens;
      } catch (error) {
        console.error("Failed to import session:", error);
        throw error;
      }
    },
    [setAuthTokens, setIsAuthenticated]
  );

  const startDeviceAuth = useCallback(
    async (
      clientId: string,
      clientSecret: string
    ): Promise<DeviceAuthResponse> => {
      try {
        return await invoke<DeviceAuthResponse>("start_device_auth", {
          clientId,
          clientSecret,
        });
      } catch (error) {
        console.error("Failed to start device auth:", error);
        throw error;
      }
    },
    []
  );

  const pollDeviceAuth = useCallback(
    async (
      deviceCode: string,
      clientId: string,
      clientSecret: string
    ): Promise<AuthTokens | null> => {
      try {
        const result = await invoke<AuthTokens | null>("poll_device_auth", {
          deviceCode,
          clientId,
          clientSecret,
        });

        if (result) {
          let userId = result.user_id;
          if (!userId) {
            try {
              userId = await invoke<number>("get_session_user_id");
            } catch (e) {
              console.error("Failed to get user ID:", e);
            }
          }
          const updatedTokens = { ...result, user_id: userId };
          setAuthTokens(updatedTokens);
          setIsAuthenticated(true);
          return updatedTokens;
        }

        return null;
      } catch (error) {
        console.error("Failed to poll device auth:", error);
        throw error;
      }
    },
    [setAuthTokens, setIsAuthenticated]
  );

  const startPkceAuth = useCallback(
    async (clientId: string): Promise<PkceAuthParams> => {
      try {
        return await invoke<PkceAuthParams>("start_pkce_auth", { clientId });
      } catch (error) {
        console.error("Failed to start PKCE auth:", error);
        throw error;
      }
    },
    []
  );

  const completePkceAuth = useCallback(
    async (
      code: string,
      codeVerifier: string,
      clientUniqueKey: string,
      clientId: string,
      clientSecret: string
    ): Promise<AuthTokens> => {
      try {
        const tokens = await invoke<AuthTokens>("complete_pkce_auth", {
          code,
          codeVerifier,
          clientUniqueKey,
          clientId,
          clientSecret,
        });

        let userId = tokens.user_id;
        if (!userId) {
          try {
            userId = await invoke<number>("get_session_user_id");
          } catch (e) {
            console.error("Failed to get user ID:", e);
          }
        }

        const updatedTokens = { ...tokens, user_id: userId };
        setAuthTokens(updatedTokens);
        setIsAuthenticated(true);
        return updatedTokens;
      } catch (error) {
        console.error("Failed to complete PKCE auth:", error);
        throw error;
      }
    },
    [setAuthTokens, setIsAuthenticated]
  );

  const logout = useCallback(async () => {
    try {
      await invoke("logout");
      setAuthTokens(null);
      setIsAuthenticated(false);
      setUserPlaylists([]);
      setFavoritePlaylists([]);
      setCurrentTrack(null);
      setIsPlaying(false);
      setQueue([]);
      setHistory([]);
      setFavoriteTrackIds(new Set());
      try {
        localStorage.removeItem(PLAYBACK_STATE_KEY);
        localStorage.removeItem(VOLUME_STATE_KEY);
      } catch (err) {
        console.error("Failed to clear playback state:", err);
      }
    } catch (error) {
      console.error("Failed to logout:", error);
    }
  }, [
    setAuthTokens,
    setIsAuthenticated,
    setUserPlaylists,
    setFavoritePlaylists,
    setCurrentTrack,
    setIsPlaying,
    setQueue,
    setHistory,
    setFavoriteTrackIds,
  ]);

  const getUserPlaylists = useCallback(
    async (userId: number): Promise<Playlist[]> => {
      try {
        const playlists = await invoke<Playlist[]>("get_user_playlists", {
          userId: userId,
        });
        setUserPlaylists(playlists);
        return playlists;
      } catch (error) {
        console.error("Failed to get playlists:", error);
        return [];
      }
    },
    [setUserPlaylists]
  );

  return {
    isAuthenticated,
    authTokens,
    userName,
    importSession,
    startDeviceAuth,
    pollDeviceAuth,
    startPkceAuth,
    completePkceAuth,
    logout,
    getUserPlaylists,
  };
}
