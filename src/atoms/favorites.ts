import { atom } from "jotai";

export const favoriteTrackIdsAtom = atom<Set<number>>(new Set<number>());
