import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { X, Loader2, Search, Play } from "lucide-react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";
import { OutputPass } from "three/examples/jsm/postprocessing/OutputPass.js";
import {
  getTidalImageUrl,
  type Track,
  type PaginatedTracks,
  type AllPlaylistsResponse,
} from "../types";
import { queueAtom, currentTrackAtom } from "../atoms/playback";

interface Props {
  open: boolean;
  onClose: () => void;
}

interface AlbumNode {
  id: number;
  title: string;
  cover?: string;
  vibrantColor?: string;
  artistId: number;
  artistName: string;
  year?: number;
  decade: number; // 1960, 1970, …, 0 = unknown
  tracks: Track[];
  center: THREE.Vector3;
}

interface ArtistNode {
  id: number;
  name: string;
  hue: number;
  decade: number;
  albums: AlbumNode[];
  center: THREE.Vector3;
}

interface TrackStar {
  track: Track;
  album: AlbumNode;
  artist: ArtistNode;
  position: THREE.Vector3;
}

interface LayoutResult {
  stars: TrackStar[];
  albums: AlbumNode[];
  artists: ArtistNode[];
  decades: number[];
}

const MAX_STARS = 1400;
const MAX_ALBUMS = 500;
const COVER_SIZE_PX = 160;
const COVER_TEX_SIZE = 128;
const COVER_LOAD_CONCURRENCY = 12;

export default function LibraryGalaxy({ open, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const setQueue = useSetAtom(queueAtom);
  const setCurrentTrack = useSetAtom(currentTrackAtom);
  const [loading, setLoading] = useState(true);
  const [loadingLabel, setLoadingLabel] = useState("Trazando tu galaxia…");
  const [error, setError] = useState<string | null>(null);
  const [layout, setLayout] = useState<LayoutResult | null>(null);
  const [hovered, setHovered] = useState<TrackStar | null>(null);
  const [search, setSearch] = useState("");
  const flyToAlbumRef = useRef<((album: AlbumNode) => void) | null>(null);

  // ── Data load ────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setLayout(null);

    (async () => {
      try {
        setLoadingLabel("Leyendo tus favoritos…");
        const userId = await invoke<number>("get_session_user_id");

        const favTracks = await loadAllFavoriteTracks(userId);
        if (cancelled) return;

        setLoadingLabel("Recorriendo tus playlists…");
        const playlistTracks = await loadUserPlaylistTracks(userId);
        if (cancelled) return;

        // Merge + dedupe by track id; favorites win on collision.
        const byId = new Map<number, Track>();
        for (const t of favTracks) if (t?.id) byId.set(t.id, t);
        for (const t of playlistTracks) if (t?.id && !byId.has(t.id)) byId.set(t.id, t);

        setLoadingLabel("Mapeando tu cosmos…");
        const built = buildLayout([...byId.values()]);
        if (cancelled) return;

        setLayout(built);
        setLoading(false);
      } catch (e) {
        if (cancelled) return;
        setError(typeof e === "string" ? e : ((e as { message?: string })?.message ?? "Error"));
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [open]);

  // Esc closes
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  // ── Three scene ──────────────────────────────────────────────────────────
  useEffect(() => {
    if (!open || loading || !layout || layout.stars.length === 0 || !containerRef.current) {
      return;
    }

    const container = containerRef.current;
    const width = container.clientWidth;
    const height = container.clientHeight;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#04040a");
    scene.fog = new THREE.FogExp2(0x04040a, 0.0016);

    const camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 4000);
    camera.position.set(0, 320, 720);

    const renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    container.appendChild(renderer.domElement);

    const composer = new EffectComposer(renderer);
    composer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    composer.setSize(width, height);
    composer.addPass(new RenderPass(scene, camera));
    const bloomPass = new UnrealBloomPass(
      new THREE.Vector2(width, height),
      0.32,
      0.7,
      0.62,
    );
    composer.addPass(bloomPass);
    composer.addPass(new OutputPass());

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.07;
    controls.minDistance = 14;
    controls.maxDistance = 900;
    controls.rotateSpeed = 0.55;
    controls.zoomSpeed = 0.9;
    controls.autoRotate = true;
    controls.autoRotateSpeed = 0.1;

    scene.add(new THREE.AmbientLight(0xffffff, 0.35));

    // Background — far cosmic dust + soft nebula tints per decade arm.
    scene.add(buildBackgroundField(2200));
    for (const d of layout.decades) {
      scene.add(buildDecadeNebula(d, layout.decades));
    }

    // Constellation lines connecting albums of the same artist.
    const linesGroup = buildConstellationLines(layout.artists);
    scene.add(linesGroup);

    // ── Per-album cover materials (shared across track-sprites of same album)
    const haloTex = makeHaloTexture();
    const placeholderCache = new Map<string, THREE.Texture>();
    const albumMaterials = new Map<number, THREE.SpriteMaterial>();
    const haloSprites = new Map<number, THREE.Sprite>();
    const albumGroup = new THREE.Group();

    for (const album of layout.albums) {
      const colorHex = album.vibrantColor
        ? `#${album.vibrantColor}`
        : artistColor(album.artistName);
      const tint = new THREE.Color(colorHex);

      // Halo: one per album cluster, behind all its track-sprites.
      const haloMat = new THREE.SpriteMaterial({
        map: haloTex,
        color: tint,
        transparent: true,
        opacity: 0.55,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      });
      const halo = new THREE.Sprite(haloMat);
      const haloSize = 10 + Math.min(14, Math.sqrt(album.tracks.length) * 5);
      halo.scale.setScalar(haloSize);
      halo.position.copy(album.center);
      albumGroup.add(halo);
      haloSprites.set(album.id, halo);

      // Cover material — placeholder until the real cover loads.
      const placeholder = getOrBuildPlaceholder(placeholderCache, colorHex, album.title);
      const mat = new THREE.SpriteMaterial({
        map: placeholder,
        transparent: true,
        depthWrite: false,
      });
      albumMaterials.set(album.id, mat);
    }

    // ── Track-stars: one sprite per track, sharing its album material.
    const sprites: THREE.Sprite[] = [];
    const baseStarSize = 3.6;
    for (const star of layout.stars) {
      const mat = albumMaterials.get(star.album.id)!;
      const s = new THREE.Sprite(mat);
      s.scale.setScalar(baseStarSize);
      s.position.copy(star.position);
      s.userData.star = star;
      albumGroup.add(s);
      sprites.push(s);
    }
    scene.add(albumGroup);

    // Async load real covers (concurrency-limited) into shared materials.
    const loadedTextures: THREE.Texture[] = [];
    let aliveCovers = true;
    void loadCoversIntoMaterials(
      layout.albums,
      albumMaterials,
      (tex) => loadedTextures.push(tex),
      () => aliveCovers,
    );

    // ── Picking ────────────────────────────────────────────────────────────
    const raycaster = new THREE.Raycaster();
    const mouseNDC = new THREE.Vector2();
    let hoveredSprite: THREE.Sprite | null = null;

    const updateMouse = (e: MouseEvent) => {
      const rect = renderer.domElement.getBoundingClientRect();
      mouseNDC.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      mouseNDC.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    };

    const pickSprite = (): THREE.Sprite | null => {
      raycaster.setFromCamera(mouseNDC, camera);
      const hits = raycaster.intersectObjects(sprites, false);
      if (hits.length === 0) return null;
      // Prefer the closest sprite to camera.
      let best = hits[0];
      for (const h of hits) if (h.distance < best.distance) best = h;
      return best.object as THREE.Sprite;
    };

    const onMove = (e: MouseEvent) => {
      updateMouse(e);
      const s = pickSprite();
      if (s !== hoveredSprite) {
        hoveredSprite = s;
        setHovered(s ? (s.userData.star as TrackStar) : null);
        renderer.domElement.style.cursor = s ? "pointer" : "grab";
      }
    };

    const onClick = (e: MouseEvent) => {
      updateMouse(e);
      const s = pickSprite();
      if (!s) return;
      const star = s.userData.star as TrackStar;
      playFromTrack(star);
    };

    const playFromTrack = (star: TrackStar) => {
      // Queue the album in chronological order, starting from the clicked track.
      const ordered = [...star.album.tracks].sort((a, b) => {
        const at = (a.volumeNumber ?? 1) * 1000 + (a.trackNumber ?? 0);
        const bt = (b.volumeNumber ?? 1) * 1000 + (b.trackNumber ?? 0);
        return at - bt;
      });
      const startIdx = Math.max(0, ordered.findIndex((t) => t.id === star.track.id));
      const queue = [...ordered.slice(startIdx), ...ordered.slice(0, startIdx)];
      if (queue.length === 0) return;
      setCurrentTrack(queue[0]);
      setQueue(queue);
      invoke("play_tidal_track", {
        trackId: queue[0].id,
        useTrackGain: true,
      }).catch(() => {});
    };

    renderer.domElement.addEventListener("mousemove", onMove);
    renderer.domElement.addEventListener("click", onClick);
    renderer.domElement.style.cursor = "grab";

    const stopAutoRotate = () => (controls.autoRotate = false);
    controls.addEventListener("start", stopAutoRotate);

    const onResize = () => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
      composer.setSize(w, h);
      bloomPass.resolution.set(w, h);
    };
    const ro = new ResizeObserver(onResize);
    ro.observe(container);

    // Camera fly-in.
    const introStart = performance.now();
    const introFrom = camera.position.clone();
    const introTo = new THREE.Vector3(0, 110, 280);
    const introDur = 1400;

    let flyAnim: { from: THREE.Vector3; to: THREE.Vector3; targetTo: THREE.Vector3; t0: number; dur: number } | null = null;
    flyToAlbumRef.current = (album: AlbumNode) => {
      controls.autoRotate = false;
      const dir = album.center.clone().normalize();
      const dist = 28;
      const camTo = album.center.clone().add(dir.multiplyScalar(dist)).setY(album.center.y + 16);
      flyAnim = {
        from: camera.position.clone(),
        to: camTo,
        targetTo: album.center.clone(),
        t0: performance.now(),
        dur: 900,
      };
    };

    let raf = 0;
    const tick = () => {
      const now = performance.now();
      const introT = Math.min(1, (now - introStart) / introDur);
      if (introT < 1) {
        const e = easeOutCubic(introT);
        camera.position.lerpVectors(introFrom, introTo, e);
      }
      if (flyAnim) {
        const ft = Math.min(1, (now - flyAnim.t0) / flyAnim.dur);
        const e = easeOutCubic(ft);
        camera.position.lerpVectors(flyAnim.from, flyAnim.to, e);
        controls.target.lerpVectors(controls.target.clone(), flyAnim.targetTo, e);
        if (ft >= 1) flyAnim = null;
      }

      // Subtle breathing on halos.
      const breathe = 1 + Math.sin(now * 0.0008) * 0.04;
      for (const album of layout.albums) {
        const halo = haloSprites.get(album.id);
        if (!halo) continue;
        const baseSize = 14 + Math.min(20, Math.sqrt(album.tracks.length) * 7);
        halo.scale.setScalar(baseSize * breathe);
      }

      // Hovered track-star bumps in scale; siblings of same album get a small lift too.
      const hoveredAlbumId =
        hoveredSprite ? (hoveredSprite.userData.star as TrackStar).album.id : -1;
      for (const s of sprites) {
        const star = s.userData.star as TrackStar;
        let target = baseStarSize;
        if (s === hoveredSprite) target = baseStarSize * 1.6;
        else if (star.album.id === hoveredAlbumId) target = baseStarSize * 1.15;
        const cur = s.scale.x;
        s.scale.setScalar(cur + (target - cur) * 0.18);
      }

      controls.update();
      composer.render();
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      aliveCovers = false;
      cancelAnimationFrame(raf);
      ro.disconnect();
      renderer.domElement.removeEventListener("mousemove", onMove);
      renderer.domElement.removeEventListener("click", onClick);
      controls.removeEventListener("start", stopAutoRotate);
      controls.dispose();
      // Materials are shared per album, dispose once.
      for (const mat of albumMaterials.values()) mat.dispose();
      for (const halo of haloSprites.values()) halo.material.dispose();
      for (const tex of loadedTextures) tex.dispose();
      for (const tex of placeholderCache.values()) tex.dispose();
      haloTex.dispose();
      linesGroup.traverse((obj) => {
        if (obj instanceof THREE.LineSegments) {
          obj.geometry.dispose();
          (obj.material as THREE.Material).dispose();
        }
      });
      composer.dispose();
      renderer.dispose();
      if (renderer.domElement.parentElement === container) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [open, loading, layout, setCurrentTrack, setQueue]);

  // ── Search match ─────────────────────────────────────────────────────────
  const searchMatches = useMemo(() => {
    if (!layout || !search.trim()) return [];
    const q = search.trim().toLowerCase();
    return layout.albums
      .filter(
        (a) =>
          a.title.toLowerCase().includes(q) ||
          a.artistName.toLowerCase().includes(q),
      )
      .slice(0, 8);
  }, [search, layout]);

  if (!open) return null;

  const stats = layout
    ? {
        stars: layout.stars.length,
        albums: layout.albums.length,
        artists: layout.artists.length,
        decades: layout.decades.filter((d) => d > 0).length,
      }
    : null;

  return (
    <div className="fixed inset-0 z-[60] bg-black animate-fadeIn">
      <div ref={containerRef} className="absolute inset-0" />

      {loading && (
        <div className="absolute inset-0 flex flex-col items-center justify-center text-white/70 text-[13px] gap-3 pointer-events-none">
          <Loader2 size={18} className="animate-spin" />
          <div className="tracking-wider">{loadingLabel}</div>
        </div>
      )}

      {error && (
        <div className="absolute inset-0 flex items-center justify-center text-red-400 text-[13px] pointer-events-none">
          {error}
        </div>
      )}

      {!loading && !error && layout && layout.stars.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center text-white/60 text-[13px] pointer-events-none px-8 text-center">
          Aún no hay nada en tu librería. Da like a algunas pistas o crea una playlist para llenar la galaxia.
        </div>
      )}

      {!loading && stats && (
        <div className="absolute top-5 left-5 pointer-events-none select-none">
          <div className="text-white/90 text-[12px] uppercase tracking-[0.32em] font-medium">
            Tu galaxia musical
          </div>
          <div className="mt-1 text-white/45 text-[10.5px] tracking-[0.18em]">
            {stats.stars} canciones · {stats.albums} álbumes · {stats.artists} artistas · {stats.decades} décadas
          </div>
        </div>
      )}

      <button
        onClick={onClose}
        className="absolute top-5 right-5 w-10 h-10 rounded-full flex items-center justify-center bg-white/5 hover:bg-white/15 text-white/80 hover:text-white backdrop-blur-md transition-colors border border-white/10"
        aria-label="Salir"
      >
        <X size={20} />
      </button>

      {!loading && layout && layout.stars.length > 0 && (
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 w-[min(440px,80vw)] pointer-events-auto">
          <div className="relative">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-white/50"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Buscar en tu galaxia…"
              className="w-full pl-9 pr-3 py-2 bg-white/5 hover:bg-white/8 focus:bg-white/10 border border-white/10 focus:border-white/25 rounded-full text-white text-[12.5px] placeholder-white/35 backdrop-blur-md outline-none transition-colors"
            />
          </div>
          {searchMatches.length > 0 && (
            <div className="mt-2 max-h-72 overflow-y-auto rounded-2xl bg-black/55 backdrop-blur-xl border border-white/10 divide-y divide-white/5 shadow-2xl">
              {searchMatches.map((a) => (
                <button
                  key={a.id}
                  onClick={() => {
                    flyToAlbumRef.current?.(a);
                    setSearch("");
                  }}
                  className="w-full px-3 py-2 flex items-center gap-3 text-left hover:bg-white/5 transition-colors"
                >
                  <div
                    className="w-9 h-9 rounded-md flex-shrink-0 bg-cover bg-center"
                    style={{
                      backgroundColor: a.vibrantColor ? `#${a.vibrantColor}` : "#222",
                      backgroundImage: a.cover
                        ? `url(${getTidalImageUrl(a.cover, 160)})`
                        : undefined,
                    }}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="text-white text-[12.5px] truncate">{a.title}</div>
                    <div className="text-white/50 text-[11px] truncate">
                      {a.artistName}
                      {a.year ? ` · ${a.year}` : ""}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {hovered && !loading && (
        <div className="absolute bottom-24 left-1/2 -translate-x-1/2 pointer-events-none">
          <div className="flex items-center gap-3 px-3.5 py-2.5 bg-black/65 backdrop-blur-xl rounded-2xl border border-white/10 shadow-2xl">
            <div
              className="w-12 h-12 rounded-lg flex-shrink-0 bg-cover bg-center"
              style={{
                backgroundColor: hovered.album.vibrantColor
                  ? `#${hovered.album.vibrantColor}`
                  : "#222",
                backgroundImage: hovered.album.cover
                  ? `url(${getTidalImageUrl(hovered.album.cover, 160)})`
                  : undefined,
              }}
            />
            <div className="min-w-0 max-w-[280px]">
              <div className="text-white text-[13px] font-medium truncate">
                {hovered.track.title}
              </div>
              <div className="text-white/65 text-[11.5px] truncate">
                {hovered.album.artistName}
              </div>
              <div className="mt-0.5 text-white/40 text-[10.5px] tracking-wide truncate">
                {hovered.album.title}
                {hovered.album.year ? ` · ${hovered.album.year}` : ""}
              </div>
            </div>
            <div className="ml-1 w-7 h-7 rounded-full flex items-center justify-center bg-white/10 text-white/85">
              <Play size={12} className="ml-[1px]" />
            </div>
          </div>
        </div>
      )}

      {!loading && !hovered && (
        <p className="absolute top-5 left-1/2 -translate-x-1/2 text-[10px] uppercase tracking-[0.3em] text-white/25 select-none pointer-events-none">
          arrastra para girar · rueda para zoom · click en una estrella para reproducir · esc salir
        </p>
      )}
    </div>
  );
}

// ─── Data loading ───────────────────────────────────────────────────────────

async function loadAllFavoriteTracks(userId: number): Promise<Track[]> {
  const all: Track[] = [];
  let offset = 0;
  const pageSize = 200;
  const maxTracks = 2000;
  while (all.length < maxTracks) {
    const page = await invoke<PaginatedTracks>("get_favorite_tracks", {
      userId,
      offset,
      limit: pageSize,
      order: "DATE",
      orderDirection: "DESC",
    });
    const items = page.items ?? [];
    if (items.length === 0) break;
    all.push(...items);
    if (items.length < pageSize) break;
    offset += pageSize;
  }
  return all;
}

async function loadUserPlaylistTracks(userId: number): Promise<Track[]> {
  let playlists: { uuid: string }[] = [];
  try {
    const resp = await invoke<AllPlaylistsResponse>("get_all_playlists", {
      userId,
      offset: 0,
      limit: 50,
      order: "DATE_UPDATED",
      orderDirection: "DESC",
    });
    playlists = (resp.items ?? [])
      .filter((p) => p.creator?.id === userId)
      .slice(0, 25)
      .map((p) => ({ uuid: p.uuid }));
  } catch {
    return [];
  }

  const result: Track[] = [];
  const queue = [...playlists];
  const concurrency = 4;
  const workers = Array.from({ length: concurrency }, async () => {
    while (queue.length > 0) {
      const next = queue.shift();
      if (!next) break;
      try {
        const tracks = await invoke<Track[]>("get_playlist_tracks", {
          playlistId: next.uuid,
        });
        if (Array.isArray(tracks)) result.push(...tracks);
      } catch {
        // skip broken playlist
      }
    }
  });
  await Promise.all(workers);
  return result;
}

// ─── Hierarchy & layout ─────────────────────────────────────────────────────

function buildLayout(tracks: Track[]): LayoutResult {
  // Bucket by album (require an album id).
  const albumMap = new Map<number, AlbumNode>();
  for (const t of tracks) {
    if (!t.album?.id) continue;
    const id = t.album.id;
    let node = albumMap.get(id);
    if (!node) {
      const year = parseYear(t.album.releaseDate);
      const decade = year ? Math.floor(year / 10) * 10 : 0;
      node = {
        id,
        title: t.album.title ?? "—",
        cover: t.album.cover,
        vibrantColor: t.album.vibrantColor,
        artistId: t.artist?.id ?? -1,
        artistName: t.artist?.name ?? "—",
        year,
        decade,
        tracks: [],
        center: new THREE.Vector3(),
      };
      albumMap.set(id, node);
    }
    node.tracks.push(t);
  }

  // Cap albums to the most-tracked first, so a 2000-track library still renders well.
  let albums = [...albumMap.values()].sort((a, b) => b.tracks.length - a.tracks.length);
  if (albums.length > MAX_ALBUMS) albums = albums.slice(0, MAX_ALBUMS);

  // Group into artists.
  const artistMap = new Map<number, ArtistNode>();
  for (const album of albums) {
    let a = artistMap.get(album.artistId);
    if (!a) {
      a = {
        id: album.artistId,
        name: album.artistName,
        hue: hueFromString(album.artistName),
        decade: album.decade,
        albums: [],
        center: new THREE.Vector3(),
      };
      artistMap.set(album.artistId, a);
    }
    a.albums.push(album);
  }
  const artists = [...artistMap.values()];
  for (const a of artists) {
    const ds = a.albums.map((al) => al.decade).filter((d) => d > 0).sort();
    a.decade = ds.length > 0 ? ds[Math.floor(ds.length / 2)] : 0;
  }

  // Group artists into decades.
  const decadeMap = new Map<number, ArtistNode[]>();
  for (const a of artists) {
    const list = decadeMap.get(a.decade);
    if (list) list.push(a);
    else decadeMap.set(a.decade, [a]);
  }
  const decades = [...decadeMap.keys()].sort((a, b) => {
    if (a === 0) return 1;
    if (b === 0) return -1;
    return a - b;
  });

  // ── Spatial layout ──────────────────────────────────────────────────────
  // Galactic disk: each decade owns an angular sector. Within a sector,
  // artists ride a logarithmic spiral arm from the core outward, with
  // popular artists nearer the core. Each artist hosts a tight ring of
  // album clusters; each album is a tight cloud of track-stars.
  const sectorCount = decades.length;
  const baseRadius = 60;
  const outerRadius = 320;

  decades.forEach((decade, di) => {
    const sectorCenter = (di / Math.max(1, sectorCount)) * Math.PI * 2;
    const sectorHalfWidth = (Math.PI * 2) / Math.max(1, sectorCount) * 0.4;
    const decadeArtists = decadeMap.get(decade)!;

    decadeArtists.sort(
      (a, b) =>
        b.albums.reduce((s, x) => s + x.tracks.length, 0) -
        a.albums.reduce((s, x) => s + x.tracks.length, 0),
    );

    const N = decadeArtists.length;
    decadeArtists.forEach((artist, ai) => {
      const t = N === 1 ? 0.5 : ai / (N - 1);
      const radius = baseRadius + Math.pow(t, 0.85) * (outerRadius - baseRadius);
      const armCurve = (radius - baseRadius) * 0.0048;
      const angle =
        sectorCenter +
        (t - 0.5) * sectorHalfWidth * 1.7 +
        armCurve;
      const yJitter = (hashFloat(artist.id, 1) - 0.5) * (10 + (radius - baseRadius) * 0.08);

      artist.center.set(
        Math.cos(angle) * radius,
        yJitter,
        Math.sin(angle) * radius,
      );

      artist.albums.sort((a, b) => (a.year ?? 0) - (b.year ?? 0));

      // Albums orbit the artist center on a small ring.
      const M = artist.albums.length;
      const ringR = 6 + Math.min(14, M * 1.4);
      artist.albums.forEach((album, mi) => {
        const phase = (mi / Math.max(1, M)) * Math.PI * 2 + hashFloat(album.id, 2) * 0.4;
        const localR = ringR * (0.65 + hashFloat(album.id, 3) * 0.55);
        const localY = (hashFloat(album.id, 4) - 0.5) * 4.5;
        album.center.set(
          artist.center.x + Math.cos(phase) * localR,
          artist.center.y + localY,
          artist.center.z + Math.sin(phase) * localR,
        );
      });
    });
  });

  // ── Build track-stars ───────────────────────────────────────────────────
  // Each track sits in a tiny cloud around its album's center. Cluster
  // radius scales gently with track count so 30-track albums stay legible.
  const stars: TrackStar[] = [];
  let totalStars = 0;
  for (const artist of artists) {
    for (const album of artist.albums) {
      const K = album.tracks.length;
      const cloudR = 1.6 + Math.min(4.2, Math.sqrt(K) * 0.85);
      // Sort tracks by trackNumber so the cluster has a stable shape.
      const ordered = [...album.tracks].sort(
        (a, b) =>
          (a.volumeNumber ?? 1) * 1000 +
          (a.trackNumber ?? 0) -
          ((b.volumeNumber ?? 1) * 1000 + (b.trackNumber ?? 0)),
      );
      ordered.forEach((track, ti) => {
        if (totalStars >= MAX_STARS) return;
        // Fibonacci-like distribution inside a small sphere.
        const u = (ti + 0.5) / Math.max(1, K);
        const phi = Math.acos(1 - 2 * u);
        const theta = Math.PI * (1 + Math.sqrt(5)) * ti;
        const r = cloudR * (0.55 + hashFloat(track.id, 5) * 0.5);
        const offset = new THREE.Vector3(
          Math.sin(phi) * Math.cos(theta) * r,
          Math.cos(phi) * r * 0.7,
          Math.sin(phi) * Math.sin(theta) * r,
        );
        stars.push({
          track,
          album,
          artist,
          position: album.center.clone().add(offset),
        });
        totalStars++;
      });
    }
  }

  return { stars, albums, artists, decades };
}

// ─── Visual helpers ─────────────────────────────────────────────────────────

function buildConstellationLines(artists: ArtistNode[]): THREE.Group {
  const g = new THREE.Group();
  for (const artist of artists) {
    if (artist.albums.length < 2) continue;
    const pts: number[] = [];
    const cols: number[] = [];
    const tint = new THREE.Color().setHSL(artist.hue, 0.55, 0.55);
    for (let i = 0; i < artist.albums.length - 1; i++) {
      const a = artist.albums[i].center;
      const b = artist.albums[i + 1].center;
      pts.push(a.x, a.y, a.z, b.x, b.y, b.z);
      cols.push(tint.r, tint.g, tint.b, tint.r, tint.g, tint.b);
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pts), 3));
    geom.setAttribute("color", new THREE.BufferAttribute(new Float32Array(cols), 3));
    const mat = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: 0.2,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    g.add(new THREE.LineSegments(geom, mat));
  }
  return g;
}

function buildBackgroundField(count: number): THREE.Points {
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  const tmp = new THREE.Color();
  for (let i = 0; i < count; i++) {
    const r = 800 + Math.random() * 600;
    const u = Math.random();
    const v = Math.random();
    const theta = 2 * Math.PI * u;
    const phi = Math.acos(2 * v - 1);
    positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
    positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
    positions[i * 3 + 2] = r * Math.cos(phi);
    tmp.setHSL(0.55 + Math.random() * 0.15, 0.5, 0.6 + Math.random() * 0.3);
    colors[i * 3] = tmp.r;
    colors[i * 3 + 1] = tmp.g;
    colors[i * 3 + 2] = tmp.b;
  }
  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geom.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  const mat = new THREE.PointsMaterial({
    size: 0.9,
    vertexColors: true,
    transparent: true,
    opacity: 0.55,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });
  return new THREE.Points(geom, mat);
}

function buildDecadeNebula(decade: number, allDecades: number[]): THREE.Sprite {
  const idx = allDecades.indexOf(decade);
  const sectorCenter = (idx / Math.max(1, allDecades.length)) * Math.PI * 2;
  const radius = 220;
  const hue = (idx / Math.max(1, allDecades.length)) % 1;

  const size = 256;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const ctx = c.getContext("2d")!;
  const grad = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  const col = new THREE.Color().setHSL(hue, 0.7, 0.5);
  grad.addColorStop(0, `rgba(${(col.r * 255) | 0},${(col.g * 255) | 0},${(col.b * 255) | 0},0.5)`);
  grad.addColorStop(0.4, `rgba(${(col.r * 255) | 0},${(col.g * 255) | 0},${(col.b * 255) | 0},0.16)`);
  grad.addColorStop(1, "rgba(0,0,0,0)");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, size, size);

  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mat = new THREE.SpriteMaterial({
    map: tex,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    opacity: 0.5,
  });
  const sprite = new THREE.Sprite(mat);
  sprite.position.set(
    Math.cos(sectorCenter) * radius,
    -10,
    Math.sin(sectorCenter) * radius,
  );
  sprite.scale.setScalar(320);
  return sprite;
}

function makeHaloTexture(): THREE.Texture {
  const size = 128;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const ctx = c.getContext("2d")!;
  const grad = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  grad.addColorStop(0, "rgba(255,255,255,0.95)");
  grad.addColorStop(0.18, "rgba(255,255,255,0.45)");
  grad.addColorStop(0.55, "rgba(255,255,255,0.08)");
  grad.addColorStop(1, "rgba(255,255,255,0)");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

function getOrBuildPlaceholder(
  cache: Map<string, THREE.Texture>,
  hex: string,
  title: string,
): THREE.Texture {
  const key = `${hex}|${title.charAt(0).toUpperCase()}`;
  const found = cache.get(key);
  if (found) return found;

  const size = COVER_TEX_SIZE;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const ctx = c.getContext("2d")!;

  const r = size * 0.16;
  ctx.beginPath();
  roundRect(ctx, 0, 0, size, size, r);
  ctx.closePath();
  const grad = ctx.createLinearGradient(0, 0, size, size);
  grad.addColorStop(0, lighten(hex, 0.15));
  grad.addColorStop(1, darken(hex, 0.25));
  ctx.fillStyle = grad;
  ctx.fill();

  ctx.fillStyle = "rgba(255,255,255,0.75)";
  ctx.font = `${Math.floor(size * 0.5)}px ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(title.charAt(0).toUpperCase() || "·", size / 2, size / 2 + size * 0.04);

  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 4;
  cache.set(key, tex);
  return tex;
}

async function loadCoversIntoMaterials(
  albums: AlbumNode[],
  materials: Map<number, THREE.SpriteMaterial>,
  trackTexture: (tex: THREE.Texture) => void,
  alive: () => boolean,
) {
  const queue = albums.filter((a) => a.cover && materials.has(a.id));
  let cursor = 0;

  const worker = async () => {
    while (alive()) {
      const idx = cursor++;
      if (idx >= queue.length) return;
      const album = queue[idx];
      const mat = materials.get(album.id);
      if (!mat) continue;
      try {
        const tex = await loadCoverTexture(album.cover!);
        if (!alive()) {
          tex.dispose();
          return;
        }
        mat.map = tex;
        mat.needsUpdate = true;
        trackTexture(tex);
      } catch {
        // Keep placeholder on failure.
      }
    }
  };

  await Promise.all(
    Array.from({ length: COVER_LOAD_CONCURRENCY }, () => worker()),
  );
}

function loadCoverTexture(coverUuid: string): Promise<THREE.Texture> {
  const url = getTidalImageUrl(coverUuid, COVER_SIZE_PX);
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.referrerPolicy = "no-referrer";
    img.onload = () => {
      const size = COVER_TEX_SIZE;
      const c = document.createElement("canvas");
      c.width = c.height = size;
      const ctx = c.getContext("2d")!;
      const r = size * 0.16;
      ctx.beginPath();
      roundRect(ctx, 0, 0, size, size, r);
      ctx.closePath();
      ctx.clip();
      ctx.drawImage(img, 0, 0, size, size);

      const sheen = ctx.createLinearGradient(0, 0, 0, size * 0.25);
      sheen.addColorStop(0, "rgba(255,255,255,0.16)");
      sheen.addColorStop(1, "rgba(255,255,255,0)");
      ctx.fillStyle = sheen;
      ctx.fillRect(0, 0, size, size);

      const tex = new THREE.CanvasTexture(c);
      tex.colorSpace = THREE.SRGBColorSpace;
      tex.anisotropy = 4;
      tex.needsUpdate = true;
      resolve(tex);
    };
    img.onerror = () => reject(new Error("cover load failed"));
    img.src = url;
  });
}

// ─── Misc helpers ───────────────────────────────────────────────────────────

function parseYear(date?: string): number | undefined {
  if (!date) return undefined;
  const m = date.match(/^(\d{4})/);
  if (!m) return undefined;
  const y = parseInt(m[1], 10);
  if (!Number.isFinite(y) || y < 1900 || y > 2100) return undefined;
  return y;
}

function hueFromString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return ((h >>> 0) % 360) / 360;
}

function artistColor(name: string): string {
  const c = new THREE.Color().setHSL(hueFromString(name), 0.65, 0.6);
  return `#${c.getHexString()}`;
}

function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

function hashFloat(seed: number, salt: number): number {
  let x = (seed * 0x9e3779b1 + salt * 0x85ebca6b) >>> 0;
  x ^= x >>> 16;
  x = Math.imul(x, 0x7feb352d);
  x ^= x >>> 15;
  x = Math.imul(x, 0x846ca68b);
  x ^= x >>> 16;
  return (x >>> 0) / 0xffffffff;
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
}

function lighten(hex: string, amt: number): string {
  const c = new THREE.Color(hex);
  const hsl = { h: 0, s: 0, l: 0 };
  c.getHSL(hsl);
  hsl.l = Math.min(1, hsl.l + amt);
  c.setHSL(hsl.h, hsl.s, hsl.l);
  return `#${c.getHexString()}`;
}

function darken(hex: string, amt: number): string {
  const c = new THREE.Color(hex);
  const hsl = { h: 0, s: 0, l: 0 };
  c.getHSL(hsl);
  hsl.l = Math.max(0, hsl.l - amt);
  c.setHSL(hsl.h, hsl.s, hsl.l);
  return `#${c.getHexString()}`;
}
