import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { X, Loader2 } from "lucide-react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { Track, PaginatedTracks } from "../types";
import { queueAtom, currentTrackAtom } from "../atoms/playback";

interface Props {
  open: boolean;
  onClose: () => void;
}

interface Star {
  position: THREE.Vector3;
  artist: string;
  track: Track;
}

/** Deterministic hash → hue [0,1) for stable artist colors. */
function hueFromString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return ((h >>> 0) % 360) / 360;
}

export default function LibraryGalaxy({ open, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const setQueue = useSetAtom(queueAtom);
  const setCurrentTrack = useSetAtom(currentTrackAtom);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stars, setStars] = useState<Star[]>([]);
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  // Load favorite tracks once when the panel opens.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    (async () => {
      try {
        // Fetch a generous slice of the user's favorites. Most libraries fit.
        const all: Track[] = [];
        let offset = 0;
        const pageSize = 200;
        const maxTracks = 1500;
        while (all.length < maxTracks) {
          const page = await invoke<PaginatedTracks>("get_favorite_tracks", {
            limit: pageSize,
            offset,
          });
          const items = page.items ?? [];
          if (items.length === 0) break;
          all.push(...items);
          if (items.length < pageSize) break;
          offset += pageSize;
        }
        if (cancelled) return;
        setStars(buildLayout(all));
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

  // Three.js scene lifecycle.
  useEffect(() => {
    if (!open || loading || stars.length === 0 || !containerRef.current) return;

    const container = containerRef.current;
    const width = container.clientWidth;
    const height = container.clientHeight;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#06060c");
    scene.fog = new THREE.FogExp2(0x06060c, 0.002);

    const camera = new THREE.PerspectiveCamera(
      55,
      width / height,
      0.1,
      4000,
    );
    camera.position.set(0, 80, 220);

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.06;
    controls.minDistance = 12;
    controls.maxDistance = 700;
    controls.rotateSpeed = 0.6;
    controls.zoomSpeed = 0.8;
    controls.autoRotate = true;
    controls.autoRotateSpeed = 0.15;

    // Stars as Points: one draw call.
    const positions = new Float32Array(stars.length * 3);
    const colors = new Float32Array(stars.length * 3);
    const sizes = new Float32Array(stars.length);
    const tmp = new THREE.Color();
    stars.forEach((s, i) => {
      positions[i * 3] = s.position.x;
      positions[i * 3 + 1] = s.position.y;
      positions[i * 3 + 2] = s.position.z;
      tmp.setHSL(hueFromString(s.artist), 0.65, 0.6);
      colors[i * 3] = tmp.r;
      colors[i * 3 + 1] = tmp.g;
      colors[i * 3 + 2] = tmp.b;
      sizes[i] = 1.2 + Math.random() * 0.8;
    });

    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geom.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    geom.setAttribute("size", new THREE.BufferAttribute(sizes, 1));

    const sprite = makeStarSprite();
    const mat = new THREE.PointsMaterial({
      size: 2.6,
      map: sprite,
      vertexColors: true,
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
      sizeAttenuation: true,
    });

    const points = new THREE.Points(geom, mat);
    scene.add(points);

    // Faint cosmic dust: thousands of tiny background stars.
    scene.add(buildBackgroundField(2500));

    // Raycaster for click + hover.
    const raycaster = new THREE.Raycaster();
    raycaster.params.Points = { threshold: 1.5 };
    const mouseNDC = new THREE.Vector2();
    let hoveredIndex: number | null = null;

    const updateMouse = (e: MouseEvent) => {
      const rect = renderer.domElement.getBoundingClientRect();
      mouseNDC.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      mouseNDC.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    };

    const pickIndex = (): number | null => {
      raycaster.setFromCamera(mouseNDC, camera);
      const hits = raycaster.intersectObject(points);
      if (hits.length === 0) return null;
      // Take the closest hit by distance to ray origin.
      let best = hits[0];
      for (const h of hits) {
        if (h.distance < best.distance) best = h;
      }
      return typeof best.index === "number" ? best.index : null;
    };

    const onMove = (e: MouseEvent) => {
      updateMouse(e);
      const idx = pickIndex();
      if (idx !== hoveredIndex) {
        hoveredIndex = idx;
        setHoveredIdx(idx);
        renderer.domElement.style.cursor = idx !== null ? "pointer" : "grab";
      }
    };

    const onClick = (e: MouseEvent) => {
      updateMouse(e);
      const idx = pickIndex();
      if (idx === null) return;
      const star = stars[idx];
      // Build a 50-track queue around the picked star (same artist first,
      // then random fill) so playing one star turns into a real session.
      const sameArtist = stars
        .map((s) => s.track)
        .filter((t) => (t.artist?.name ?? "—") === star.artist);
      const others = stars
        .map((s) => s.track)
        .filter((t) => (t.artist?.name ?? "—") !== star.artist)
        .sort(() => Math.random() - 0.5)
        .slice(0, Math.max(0, 50 - sameArtist.length));
      const queue = [star.track, ...sameArtist.filter((t) => t.id !== star.track.id), ...others].slice(0, 50);
      setCurrentTrack(star.track);
      setQueue(queue);
      invoke("play_tidal_track", {
        trackId: star.track.id,
        useTrackGain: true,
      }).catch(() => {});
    };

    renderer.domElement.addEventListener("mousemove", onMove);
    renderer.domElement.addEventListener("click", onClick);
    renderer.domElement.style.cursor = "grab";

    // Resize handling.
    const onResize = () => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
    };
    const ro = new ResizeObserver(onResize);
    ro.observe(container);

    // Pause auto-rotate while user is dragging.
    controls.addEventListener("start", () => (controls.autoRotate = false));

    let raf = 0;
    const tick = () => {
      controls.update();
      renderer.render(scene, camera);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      renderer.domElement.removeEventListener("mousemove", onMove);
      renderer.domElement.removeEventListener("click", onClick);
      controls.dispose();
      geom.dispose();
      mat.dispose();
      sprite.dispose();
      renderer.dispose();
      if (renderer.domElement.parentElement === container) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [open, loading, stars, setCurrentTrack, setQueue]);

  const hovered = hoveredIdx != null ? stars[hoveredIdx] : null;

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[60] bg-black animate-fadeIn">
      {/* Three.js canvas mount */}
      <div ref={containerRef} className="absolute inset-0" />

      {loading && (
        <div className="absolute inset-0 flex items-center justify-center text-white/70 text-[13px] gap-2 pointer-events-none">
          <Loader2 size={14} className="animate-spin" />
          Cargando tu galaxia musical…
        </div>
      )}

      {error && (
        <div className="absolute inset-0 flex items-center justify-center text-red-400 text-[13px] pointer-events-none">
          {error}
        </div>
      )}

      {!loading && !error && stars.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center text-white/60 text-[13px] pointer-events-none px-8 text-center">
          Sin favoritos en TIDAL — añade pistas a tu librería para llenar la galaxia.
        </div>
      )}

      {/* Hover tooltip */}
      {hovered && !loading && (
        <div className="absolute bottom-12 left-1/2 -translate-x-1/2 px-4 py-2 bg-black/60 backdrop-blur-md rounded-lg text-white text-[12px] shadow-2xl pointer-events-none">
          <div className="font-medium">{hovered.track.title}</div>
          <div className="text-white/60 text-[11px]">{hovered.artist}</div>
        </div>
      )}

      {/* Header label */}
      <div className="absolute top-5 left-5 text-white/60 text-[11px] uppercase tracking-[0.25em] pointer-events-none">
        Galaxia · {stars.length} estrellas
      </div>

      <button
        onClick={onClose}
        className="absolute top-5 right-5 w-10 h-10 rounded-full flex items-center justify-center bg-black/40 hover:bg-black/60 text-white/80 hover:text-white backdrop-blur-md transition-colors"
        aria-label="Salir"
      >
        <X size={20} />
      </button>

      <p className="absolute bottom-4 left-1/2 -translate-x-1/2 text-[10px] uppercase tracking-[0.3em] text-white/30 select-none pointer-events-none">
        click a una estrella para reproducir · arrastra para girar · rueda para zoom · esc salir
      </p>
    </div>
  );
}

// ─── Layout helpers ─────────────────────────────────────────────────────

function buildLayout(tracks: Track[]): Star[] {
  // Bucket tracks per artist; each artist gets a deterministic position
  // on a Fibonacci-sphere-ish distribution so clusters spread evenly.
  const buckets = new Map<string, Track[]>();
  for (const t of tracks) {
    const key = t.artist?.name ?? "—";
    const list = buckets.get(key);
    if (list) list.push(t);
    else buckets.set(key, [t]);
  }

  const artists = [...buckets.keys()];
  const phi = Math.PI * (3 - Math.sqrt(5)); // golden angle
  const stars: Star[] = [];

  artists.forEach((artist, i) => {
    // Fibonacci sphere mapped to a cloud (random radius modulating depth).
    const t = (i + 0.5) / Math.max(1, artists.length);
    const y = 1 - 2 * t; // [-1,1]
    const r = Math.sqrt(1 - y * y);
    const theta = phi * i;
    const baseR = 60 + Math.random() * 90;
    const center = new THREE.Vector3(
      Math.cos(theta) * r * baseR,
      y * baseR * 0.6,
      Math.sin(theta) * r * baseR,
    );

    const tracks = buckets.get(artist)!;
    tracks.forEach((track) => {
      const offset = new THREE.Vector3(
        (Math.random() - 0.5) * 8,
        (Math.random() - 0.5) * 8,
        (Math.random() - 0.5) * 8,
      );
      stars.push({
        position: center.clone().add(offset),
        artist,
        track,
      });
    });
  });

  return stars;
}

function makeStarSprite(): THREE.Texture {
  const size = 64;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const ctx = c.getContext("2d")!;
  const grad = ctx.createRadialGradient(
    size / 2,
    size / 2,
    0,
    size / 2,
    size / 2,
    size / 2,
  );
  grad.addColorStop(0, "rgba(255,255,255,1)");
  grad.addColorStop(0.25, "rgba(255,255,255,0.55)");
  grad.addColorStop(0.6, "rgba(255,255,255,0.12)");
  grad.addColorStop(1, "rgba(255,255,255,0)");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(c);
  tex.needsUpdate = true;
  return tex;
}

function buildBackgroundField(count: number): THREE.Points {
  const positions = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    const r = 800 + Math.random() * 400;
    const u = Math.random();
    const v = Math.random();
    const theta = 2 * Math.PI * u;
    const phi = Math.acos(2 * v - 1);
    positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
    positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
    positions[i * 3 + 2] = r * Math.cos(phi);
  }
  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  const mat = new THREE.PointsMaterial({
    size: 0.7,
    color: 0x99aacc,
    transparent: true,
    opacity: 0.55,
    depthWrite: false,
  });
  return new THREE.Points(geom, mat);
}
