import { useCallback } from "react";
import { useAtom } from "jotai";
import { drawerOpenAtom, drawerTabAtom } from "../atoms/ui";

export function useDrawer() {
  const [drawerOpen, setDrawerOpen] = useAtom(drawerOpenAtom);
  const [drawerTab, setDrawerTab] = useAtom(drawerTabAtom);

  const toggleDrawer = useCallback(() => {
    setDrawerOpen((prev) => !prev);
  }, [setDrawerOpen]);

  const openDrawerToTab = useCallback(
    (tab: string) => {
      setDrawerTab(tab);
      setDrawerOpen(true);
    },
    [setDrawerTab, setDrawerOpen]
  );

  return {
    drawerOpen,
    drawerTab,
    setDrawerOpen,
    setDrawerTab,
    toggleDrawer,
    openDrawerToTab,
  };
}
