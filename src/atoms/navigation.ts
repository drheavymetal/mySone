import { atom } from "jotai";
import type { AppView } from "../types";

export const currentViewAtom = atom<AppView>({ type: "home" });
