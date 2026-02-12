import { useRef, useState } from "react";
import { Play, ChevronLeft, ChevronRight, User, Music } from "lucide-react";
import { useAudioContext } from "../contexts/AudioContext";
import { getTidalImageUrl, type HomeSection as HomeSectionType } from "../hooks/useAudio";

// Helpers for extracting data from the raw JSON items
// These handle both V1 (direct fields) and V2 (unwrapped from data.{}) formats
function getItemImage(item: any, size: number = 320): string {
  // Mix items: images.SMALL/MEDIUM/LARGE
  if (item.images) {
    if (typeof item.images === "object" && !Array.isArray(item.images)) {
      if (size <= 320 && item.images.SMALL?.url) return item.images.SMALL.url;
      if (size <= 640 && item.images.MEDIUM?.url) return item.images.MEDIUM.url;
      if (item.images.LARGE?.url) return item.images.LARGE.url;
      if (item.images.SMALL?.url) return item.images.SMALL.url;
    }
  }
  // V2 mix images (array of {url, width, height})
  if (item.mixImages && Array.isArray(item.mixImages) && item.mixImages.length > 0) {
    return item.mixImages[0]?.url || "";
  }
  // V2 detail images
  if (item.detailImages && typeof item.detailImages === "object" && !Array.isArray(item.detailImages)) {
    if (item.detailImages.MEDIUM?.url) return item.detailImages.MEDIUM.url;
    if (item.detailImages.SMALL?.url) return item.detailImages.SMALL.url;
  }
  if (item.detailMixImages && Array.isArray(item.detailMixImages) && item.detailMixImages.length > 0) {
    return item.detailMixImages[0]?.url || "";
  }
  // Album/playlist cover UUID
  if (item.cover) return getTidalImageUrl(item.cover, size);
  if (item.squareImage) return getTidalImageUrl(item.squareImage, size);
  if (item.image) return getTidalImageUrl(item.image, size);
  // Artist picture UUID
  if (item.picture) return getTidalImageUrl(item.picture, size);
  // Nested album cover
  if (item.album?.cover) return getTidalImageUrl(item.album.cover, size);
  // V2 imageUrl direct
  if (item.imageUrl) return item.imageUrl;
  // Video items
  if (item.imageId) return getTidalImageUrl(item.imageId, size);
  if (item.imagePath) return `https://resources.tidal.com/images/${item.imagePath.replace(/-/g, "/")}/${size}x${size}.jpg`;
  return "";
}

function getItemTitle(item: any): string {
  if (item.title) return item.title;
  if (item.name) return item.name;
  if (item.titleTextInfo?.text) return item.titleTextInfo.text;
  return "";
}

function getItemSubtitle(item: any): string {
  if (item.subTitle) return item.subTitle;
  if (item.shortSubtitle) return item.shortSubtitle;
  if (item.subtitleTextInfo?.text) return item.subtitleTextInfo.text;
  if (item.subTitleTextInfo?.text) return item.subTitleTextInfo.text;
  if (item.shortSubtitleTextInfo?.text) return item.shortSubtitleTextInfo.text;
  if (item.artist?.name) return item.artist.name;
  if (item.artists && item.artists.length > 0) return item.artists.map((a: any) => a.name).join(", ");
  if (item.creator?.name) return `By ${item.creator.name}`;
  if (item.description) return item.description;
  return "";
}

function getItemId(item: any): string {
  return item.id?.toString() || item.uuid || item.mixId || Math.random().toString(36);
}

function getItemType(item: any): string {
  return item._itemType || item.type || "";
}

function isArtistItem(item: any, sectionType: string): boolean {
  return sectionType === "ARTIST_LIST"
    || getItemType(item) === "ARTIST"
    || (item.picture !== undefined && !item.cover && !item.album && !item.images && !item.mixType);
}

function isTrackItem(item: any, sectionType: string): boolean {
  return sectionType === "TRACK_LIST"
    || getItemType(item) === "TRACK"
    || (item.duration !== undefined && item.artist !== undefined && item.album !== undefined);
}

function isMixItem(item: any, sectionType: string): boolean {
  return sectionType === "MIX_LIST"
    || getItemType(item) === "MIX"
    || item.mixType !== undefined
    || item.mixImages !== undefined;
}

interface HomeSectionProps {
  section: HomeSectionType;
}

export default function HomeSection({ section }: HomeSectionProps) {
  const {
    navigateToAlbum,
    navigateToPlaylist,
    navigateToViewAll,
    navigateToArtist,
    navigateToMix,
    playTrack,
    setQueueTracks,
  } = useAudioContext();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(true);

  const items = Array.isArray(section.items) ? section.items : [];
  if (items.length === 0) return null;

  const handleScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 10);
    setCanScrollRight(el.scrollLeft < el.scrollWidth - el.clientWidth - 10);
  };

  const scroll = (direction: "left" | "right") => {
    const el = scrollRef.current;
    if (!el) return;
    const scrollAmount = el.clientWidth * 0.8;
    el.scrollBy({
      left: direction === "left" ? -scrollAmount : scrollAmount,
      behavior: "smooth",
    });
  };

  const handleItemClick = (item: any) => {
    if (isTrackItem(item, section.sectionType)) {
      // Play the track
      const trackIndex = items.indexOf(item);
      const remainingTracks = items.slice(trackIndex + 1).filter((t: any) => isTrackItem(t, section.sectionType));
      setQueueTracks(remainingTracks);
      playTrack(item);
    } else if (isMixItem(item, section.sectionType)) {
      // Mix or radio station - navigate to mix page
      const mixId = item.mixId || item.id?.toString();
      if (mixId) {
        navigateToMix(mixId, {
          title: getItemTitle(item),
          image: getItemImage(item),
          subtitle: getItemSubtitle(item),
        });
      }
    } else if (isArtistItem(item, section.sectionType)) {
      // Artist - navigate to artist page
      const artistId = item.id;
      if (artistId) {
        navigateToArtist(artistId, {
          name: item.name || getItemTitle(item),
          picture: item.picture,
        });
      }
    } else if (item.uuid) {
      // Playlist
      navigateToPlaylist(item.uuid, {
        title: item.title,
        image: item.squareImage || item.image,
        description: item.description,
        creatorName: item.creator?.name || (item.creator?.id === 0 ? "TIDAL" : undefined),
        numberOfTracks: item.numberOfTracks,
      });
    } else if (item.id) {
      // Album (fallback for items with id that aren't mix/artist)
      navigateToAlbum(item.id, {
        title: item.title,
        cover: item.cover,
        artistName: item.artist?.name || item.artists?.[0]?.name,
      });
    }
  };

  const isTrackSection = section.sectionType === "TRACK_LIST";

  if (isTrackSection) {
    return <TrackListSection section={section} items={items} />;
  }

  return (
    <section className="mb-8">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-[22px] font-bold text-white tracking-tight hover:underline cursor-pointer">
          {section.title}
        </h2>
        <div className="flex items-center gap-2">
          {/* Scroll arrows */}
          <button
            onClick={() => scroll("left")}
            className={`w-8 h-8 rounded-full flex items-center justify-center transition-colors ${
              canScrollLeft
                ? "bg-[#2a2a2a] hover:bg-[#3a3a3a] text-white"
                : "text-[#4a4a4a] cursor-default"
            }`}
            disabled={!canScrollLeft}
          >
            <ChevronLeft size={18} />
          </button>
          <button
            onClick={() => scroll("right")}
            className={`w-8 h-8 rounded-full flex items-center justify-center transition-colors ${
              canScrollRight
                ? "bg-[#2a2a2a] hover:bg-[#3a3a3a] text-white"
                : "text-[#4a4a4a] cursor-default"
            }`}
            disabled={!canScrollRight}
          >
            <ChevronRight size={18} />
          </button>
          {section.hasMore && section.apiPath && (
            <button
              onClick={() => navigateToViewAll(section.title, section.apiPath!)}
              className="text-[13px] font-bold text-[#a6a6a6] hover:text-white uppercase tracking-wider transition-colors ml-2"
            >
              View all
            </button>
          )}
        </div>
      </div>

      {/* Horizontal scroll row */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex gap-4 overflow-x-auto no-scrollbar scroll-smooth pb-2"
      >
        {items.map((item: any) => {
          const id = getItemId(item);
          const isArtist = isArtistItem(item, section.sectionType);
          return (
            <div
              key={id}
              onClick={() => handleItemClick(item)}
              className="flex-shrink-0 w-[180px] p-3 bg-[#181818] hover:bg-[#282828] rounded-lg cursor-pointer group transition-[background-color] duration-300"
            >
              {/* Image */}
              <div
                className={`w-full aspect-square mb-3 relative overflow-hidden shadow-lg bg-[#282828] ${
                  isArtist ? "rounded-full" : "rounded-md"
                }`}
              >
                {getItemImage(item) ? (
                  <img
                    src={getItemImage(item)}
                    alt={getItemTitle(item)}
                    className="w-full h-full object-cover transform group-hover:scale-105 transition-transform duration-500 ease-out"
                    loading="lazy"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-[#333] to-[#1a1a1a]">
                    {isArtist ? (
                      <User size={40} className="text-gray-600" />
                    ) : (
                      <Music size={40} className="text-gray-600" />
                    )}
                  </div>
                )}
                {!isArtist && (
                  <>
                    <div className="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleItemClick(item);
                      }}
                      className="absolute bottom-2 right-2 w-10 h-10 bg-[#00FFFF] rounded-full flex items-center justify-center shadow-xl opacity-0 group-hover:opacity-100 translate-y-2 group-hover:translate-y-0 transition-[opacity,transform] duration-300 scale-90 group-hover:scale-100 hover:scale-110"
                    >
                      <Play size={20} fill="black" className="text-black ml-1" />
                    </button>
                  </>
                )}
              </div>
              {/* Title */}
              <h4 className={`font-bold text-[14px] text-white truncate mb-1 ${isArtist ? "text-center" : ""}`}>
                {getItemTitle(item)}
              </h4>
              {/* Subtitle */}
              {getItemSubtitle(item) && (
                <p className={`text-[12px] text-[#a6a6a6] line-clamp-2 ${isArtist ? "text-center" : ""}`}>
                  {getItemSubtitle(item)}
                </p>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

// Track list section - displayed as rows instead of cards
function TrackListSection({
  section,
  items,
}: {
  section: HomeSectionType;
  items: any[];
}) {
  const { navigateToAlbum, navigateToViewAll, playTrack, setQueueTracks } =
    useAudioContext();

  const handlePlayTrack = (item: any, index: number) => {
    const remainingTracks = items.slice(index + 1);
    setQueueTracks(remainingTracks);
    playTrack(item);
  };

  // Display as a 2-column grid of track rows
  const displayItems = items.slice(0, 8);
  const midpoint = Math.ceil(displayItems.length / 2);
  const col1 = displayItems.slice(0, midpoint);
  const col2 = displayItems.slice(midpoint);

  return (
    <section className="mb-8">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-[22px] font-bold text-white tracking-tight hover:underline cursor-pointer">
          {section.title}
        </h2>
        {section.hasMore && section.apiPath && (
          <button
            onClick={() => navigateToViewAll(section.title, section.apiPath!)}
            className="text-[13px] font-bold text-[#a6a6a6] hover:text-white uppercase tracking-wider transition-colors"
          >
            View all
          </button>
        )}
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-x-6 gap-y-1">
        {[col1, col2].map((col, colIdx) => (
          <div key={colIdx} className="flex flex-col">
            {col.map((item: any, idx: number) => {
              const globalIdx = colIdx === 0 ? idx : midpoint + idx;
              return (
                <div
                  key={getItemId(item)}
                  onClick={() => handlePlayTrack(item, globalIdx)}
                  className="flex items-center gap-3 p-2 rounded-md hover:bg-[#2a2a2a] cursor-pointer group transition-colors"
                >
                  <div className="w-10 h-10 flex-shrink-0 rounded bg-[#282828] overflow-hidden relative">
                    {getItemImage(item, 160) ? (
                      <img
                        src={getItemImage(item, 160)}
                        alt={getItemTitle(item)}
                        className="w-full h-full object-cover"
                        loading="lazy"
                      />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center">
                        <Music size={16} className="text-gray-600" />
                      </div>
                    )}
                    <div className="absolute inset-0 bg-black/40 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                      <Play size={14} fill="white" className="text-white ml-0.5" />
                    </div>
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-[14px] text-white truncate font-medium">
                      {getItemTitle(item)}
                    </p>
                    <p className="text-[12px] text-[#a6a6a6] truncate">
                      {item.artist?.name || item.artists?.[0]?.name || ""}
                      {item.followInfo && (
                        <span className="ml-1 text-[#00FFFF]">+</span>
                      )}
                    </p>
                  </div>
                  {item.album && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        navigateToAlbum(item.album.id, {
                          title: item.album.title,
                          cover: item.album.cover,
                        });
                      }}
                      className="text-[12px] text-[#666] hover:text-white truncate max-w-[120px] transition-colors hidden sm:block"
                    >
                      {item.album.title}
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </section>
  );
}
