import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { BROWSEABLE_ERAS, eraLabel, eraYearSpan } from "../../types/classical";
import type { Era } from "../../types/classical";

import EraBadge from "./EraBadge";

interface BrowsePeriodsProps {
  onBack: () => void;
}

const ERA_DESCRIPTIONS: Record<Era, string> = {
  Medieval: "Plainchant, organum, the Notre-Dame school.",
  Renaissance: "Vocal polyphony, the rise of secular music, dance forms.",
  Baroque: "Counterpoint perfected. Bach, Handel, Vivaldi, Couperin.",
  Classical: "Symphonic form, sonata-allegro. Haydn, Mozart, early Beethoven.",
  EarlyRomantic: "The bridge to programmatic music. Schubert, Chopin, Schumann.",
  Romantic: "National schools, virtuosity, expansive form. Brahms, Tchaikovsky, Verdi.",
  LateRomantic: "Maximal orchestras. Mahler, R. Strauss, Debussy, Puccini.",
  TwentiethCentury: "Modernism. Stravinsky, early Schoenberg, Bartók, Prokofiev.",
  PostWar: "Serialism, minimalism, electronics. Boulez, Reich, Pärt, Glass.",
  Contemporary: "Today's voices: Adams, Saariaho, Salonen, Adès.",
  Unknown: "",
};

/**
 * Phase 2 BrowsePeriods — a static grid of the 10 era buckets. Click on
 * a bucket and you arrive at `BrowseEra` filtered to that era.
 */
export default function BrowsePeriods({ onBack }: BrowsePeriodsProps) {
  const { navigateToClassicalEra } = useNavigation();

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
            Browse Periods
          </h1>
          <p className="mt-1 text-[13px] text-th-text-muted">
            From plainchant to today — pick an era to explore its composers.
          </p>
        </header>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {BROWSEABLE_ERAS.map((era) => (
            <button
              key={era}
              type="button"
              onClick={() => navigateToClassicalEra(era, eraLabel(era))}
              className="group flex flex-col items-start gap-3 rounded-xl border border-th-border-subtle/60 bg-th-elevated p-5 text-left transition-[background-color,border-color,transform] duration-200 hover:-translate-y-0.5 hover:border-th-accent/40 hover:bg-th-surface-hover"
            >
              <div className="flex items-center gap-3">
                <EraBadge era={era} showLabel={false} />
                <span className="text-[12px] font-mono text-th-text-muted">
                  {eraYearSpan(era)}
                </span>
              </div>
              <h2 className="text-[18px] font-bold text-th-text-primary">
                {eraLabel(era)}
              </h2>
              <p className="text-[13px] text-th-text-secondary">
                {ERA_DESCRIPTIONS[era]}
              </p>
            </button>
          ))}
        </div>
      </PageContainer>
    </div>
  );
}
