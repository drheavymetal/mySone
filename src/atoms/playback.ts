import { atom } from "jotai";
import { atomWithStorage } from "jotai/utils";
import type { Track, StreamInfo } from "../types";

export const isPlayingAtom = atom(false);
export const currentTrackAtom = atom<Track | null>(null);
export const volumeAtom = atomWithStorage("tide-vibe.volume.v1", 1.0);
export const queueAtom = atom<Track[]>([]);
export const historyAtom = atom<Track[]>([]);
export const streamInfoAtom = atom<StreamInfo | null>(null);
