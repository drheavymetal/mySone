import Sidebar from "./Sidebar";
import Header from "./Header";
import PlayerBar from "./PlayerBar";
import NowPlayingDrawer from "./NowPlayingDrawer";
import { ReactNode, useRef, useEffect } from "react";
import { useAtomValue } from "jotai";
import { currentViewAtom } from "../atoms/navigation";

interface LayoutProps {
  children: ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const currentView = useAtomValue(currentViewAtom);

  useEffect(() => {
    scrollRef.current?.scrollTo(0, 0);
  }, [currentView]);

  return (
    <div className="flex flex-col h-full w-full bg-th-overlay text-white overflow-hidden">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <div className="flex-1 flex flex-col min-w-0 bg-th-base">
          <Header />
          <div ref={scrollRef} className="flex-1 overflow-y-auto custom-scrollbar relative">
            {children}
          </div>
        </div>
      </div>
      <NowPlayingDrawer />
      <PlayerBar />
    </div>
  );
}
