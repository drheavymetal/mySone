import {
  Music,
  Music2,
  Music4,
  Mic2,
  Theater,
  Disc,
  Drum,
  Film,
  HelpCircle,
} from "lucide-react";

import PageContainer from "../PageContainer";
import type { Genre } from "../../types/classical";
import { genreLabel } from "../../types/classical";

interface BrowseGenresProps {
  onBack: () => void;
}

interface GenreEntry {
  genre: Genre;
  icon: React.ReactNode;
  description: string;
}

const ENTRIES: GenreEntry[] = [
  {
    genre: "Orchestral",
    icon: <Music size={22} />,
    description:
      "Symphonies, symphonic poems, overtures — the full orchestra at work.",
  },
  {
    genre: "Concerto",
    icon: <Music2 size={22} />,
    description: "Solo instrument and orchestra in dialogue.",
  },
  {
    genre: "Chamber",
    icon: <Music4 size={22} />,
    description: "Ensembles from duos to nonets, no conductor required.",
  },
  {
    genre: "SoloInstrumental",
    icon: <Disc size={22} />,
    description: "Piano, violin, organ, guitar — the keyboard and beyond.",
  },
  {
    genre: "Vocal",
    icon: <Mic2 size={22} />,
    description: "Lieder, mélodies, art songs.",
  },
  {
    genre: "Choral",
    icon: <Drum size={22} />,
    description: "Choirs from chamber size to massed forces.",
  },
  {
    genre: "Opera",
    icon: <Theater size={22} />,
    description: "Stage works with full dramatic apparatus.",
  },
  {
    genre: "Sacred",
    icon: <Music size={22} />,
    description: "Masses, requiems, motets, oratorios.",
  },
  {
    genre: "Stage",
    icon: <Theater size={22} />,
    description: "Ballets, incidental music, musical theatre.",
  },
  {
    genre: "Film",
    icon: <Film size={22} />,
    description: "Modern composers writing for screen.",
  },
  {
    genre: "Other",
    icon: <HelpCircle size={22} />,
    description: "Pieces that defy easy classification.",
  },
];

/**
 * Phase 2 BrowseGenres — a static grid of the 11 genre buckets. Phase 5
 * will turn each card into a clickable list of works that match the
 * genre. For now we keep them informational so the discoverability of
 * the taxonomy is in place even before the lists are filled.
 */
export default function BrowseGenres({ onBack }: BrowseGenresProps) {
  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-8">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back to Hub"
        >
          <svg
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="h-4 w-4"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
          Back
        </button>

        <header className="mb-8">
          <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
            Browse Genres
          </h1>
          <p className="mt-1 text-[13px] text-th-text-muted">
            The classical taxonomy. Drill-down by genre lands in Phase 5.
          </p>
        </header>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {ENTRIES.map((entry) => (
            <div
              key={entry.genre}
              className="flex items-start gap-4 rounded-xl border border-th-border-subtle/60 bg-th-elevated p-5"
            >
              <span
                aria-hidden="true"
                className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-th-accent/15 text-th-accent"
              >
                {entry.icon}
              </span>
              <div className="flex-1">
                <h2 className="text-[16px] font-bold text-th-text-primary">
                  {genreLabel(entry.genre)}
                </h2>
                <p className="mt-1 text-[13px] text-th-text-secondary">
                  {entry.description}
                </p>
              </div>
            </div>
          ))}
        </div>
      </PageContainer>
    </div>
  );
}
