import { atom } from "jotai";
import type { AuthTokens } from "../types";

export const isAuthenticatedAtom = atom(false);
export const authTokensAtom = atom<AuthTokens | null>(null);
export const userNameAtom = atom("Tidal User");
