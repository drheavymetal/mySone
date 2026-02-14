import { atom } from "jotai";
import type { Playlist } from "../types";

export const userPlaylistsAtom = atom<Playlist[]>([]);
export const favoritePlaylistsAtom = atom<Playlist[]>([]);
