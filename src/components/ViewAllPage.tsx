import { useState, useEffect, useRef } from "react";
import { ArrowLeft, Play, User, Music } from "lucide-react";
import { useAudioContext } from "../contexts/AudioContext";
import { getTidalImageUrl } from "../hooks/useAudio";

// Helpers for extracting data from items (handles V1 and V2 formats)
function getItemImage(item: any, size: number = 320): string {
  if (item.images && typeof item.images === "object" && !Array.isArray(item.images)) {
    if (size <= 320 && item.images.SMALL?.url) return item.images.SMALL.url;
    if (size <= 640 && item.images.MEDIUM?.url) return item.images.MEDIUM.url;
    if (item.images.LARGE?.url) return item.images.LARGE.url;
    if (item.images.SMALL?.url) return item.images.SMALL.url;
  }
  if (item.mixImages && Array.isArray(item.mixImages) && item.mixImages.length > 0) {
    return item.mixImages[0]?.url || "";
  }
  if (item.detailMixImages && Array.isArray(item.detailMixImages) && item.detailMixImages.length > 0) {
    return item.detailMixImages[0]?.url || "";
  }
  if (item.cover) return getTidalImageUrl(item.cover, size);
  if (item.squareImage) return getTidalImageUrl(item.squareImage, size);
  if (item.image) return getTidalImageUrl(item.image, size);
  if (item.picture) return getTidalImageUrl(item.picture, size);
  if (item.album?.cover) return getTidalImageUrl(item.album.cover, size);
  if (item.imageUrl) return item.imageUrl;
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

function isArtistItem(item: any): boolean {
  return getItemType(item) === "ARTIST"
    || (item.picture !== undefined && !item.cover && !item.album && !item.images && !item.mixType);
}

function isTrackItem(item: any): boolean {
  return getItemType(item) === "TRACK"
    || (item.duration !== undefined && item.artist !== undefined);
}

interface ViewAllPageProps {
  title: string;
  apiPath: string;
  onBack: () => void;
}

export default function ViewAllPage({ title, apiPath, onBack }: ViewAllPageProps) {
  const {
    getPageSection,
    navigateToAlbum,
    navigateToPlaylist,
    playTrack,
    setQueueTracks,
  } = useAudioContext();

  const [items, setItems] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const hasLoadedRef = useRef(false);

  useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;

    const loadData = async () => {
      try {
        const result = await getPageSection(apiPath);
        // Collect all items from all sections
        const allItems = result.sections.flatMap((s) =>
          Array.isArray(s.items) ? s.items : []
        );
        setItems(allItems);
      } catch (err: any) {
        console.error("Failed to load page section:", err);
        setError(err.toString());
      }
      setLoading(false);
    };

    loadData();
  }, [apiPath, getPageSection]);

  const handleItemClick = (item: any) => {
    if (isTrackItem(item)) {
      const idx = items.indexOf(item);
      setQueueTracks(items.slice(idx + 1).filter(isTrackItem));
      playTrack(item);
    } else if (item.uuid) {
      navigateToPlaylist(item.uuid, {
        title: item.title,
        image: item.squareImage || item.image,
        description: item.description,
        creatorName: item.creator?.name || (item.creator?.id === 0 ? "TIDAL" : undefined),
        numberOfTracks: item.numberOfTracks,
      });
    } else if (item.id && !isArtistItem(item)) {
      navigateToAlbum(item.id, {
        title: item.title,
        cover: item.cover,
        artistName: item.artist?.name || item.artists?.[0]?.name,
      });
    }
  };

  const hasArtists = items.length > 0 && items.every(isArtistItem);

  return (
    <div className="flex-1 bg-gradient-to-b from-[#1a1a1a] to-[#121212] min-h-full">
      <div className="px-6 py-8">
        {/* Header */}
        <div className="flex items-center gap-4 mb-8">
          <button
            onClick={onBack}
            className="w-8 h-8 rounded-full bg-black/40 flex items-center justify-center hover:bg-black/60 transition-colors"
          >
            <ArrowLeft size={18} className="text-white" />
          </button>
          <h1 className="text-[32px] font-bold text-white tracking-tight">
            {title}
          </h1>
        </div>

        {loading && (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
            {Array.from({ length: 18 }).map((_, i) => (
              <div key={i} className="p-3">
                <div className="aspect-square bg-[#282828] rounded-md animate-pulse mb-2" />
                <div className="h-4 w-32 bg-[#282828] rounded animate-pulse mb-1" />
                <div className="h-3 w-24 bg-[#282828] rounded animate-pulse" />
              </div>
            ))}
          </div>
        )}

        {error && (
          <div className="text-center py-12">
            <p className="text-[#a6a6a6] text-sm">Failed to load content</p>
            <p className="text-[#666] text-xs mt-1">{error}</p>
          </div>
        )}

        {!loading && !error && items.length === 0 && (
          <div className="text-center py-12">
            <p className="text-[#a6a6a6] text-sm">No items found</p>
          </div>
        )}

        {!loading && !error && items.length > 0 && (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
            {items.map((item: any) => {
              const id = getItemId(item);
              const isArtist = isArtistItem(item);
              return (
                <div
                  key={id}
                  onClick={() => handleItemClick(item)}
                  className="p-3 bg-[#181818] hover:bg-[#282828] rounded-lg cursor-pointer group transition-[background-color] duration-300"
                >
                  <div
                    className={`w-full aspect-square mb-3 relative overflow-hidden shadow-lg bg-[#282828] ${
                      isArtist || hasArtists ? "rounded-full" : "rounded-md"
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
                        {isArtist || hasArtists ? (
                          <User size={40} className="text-gray-600" />
                        ) : (
                          <Music size={40} className="text-gray-600" />
                        )}
                      </div>
                    )}
                    {!isArtist && !hasArtists && (
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
                  <h4
                    className={`font-bold text-[15px] text-white truncate mb-1 ${
                      isArtist || hasArtists ? "text-center" : ""
                    }`}
                  >
                    {getItemTitle(item)}
                  </h4>
                  {getItemSubtitle(item) && (
                    <p
                      className={`text-[13px] text-[#a6a6a6] line-clamp-2 ${
                        isArtist || hasArtists ? "text-center" : ""
                      }`}
                    >
                      {getItemSubtitle(item)}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
