import { useNavigation } from "../../hooks/useNavigation";
import type { Work } from "../../types/classical";

/**
 * Phase 9 (F9.9 / D-042) — right sidebar on `WorkPage` for desktop
 * widths ≥ 1280px. Three sections, all best-effort:
 *
 *   1. Related works — sourced from the parent composer's bucket
 *      catalogue (same bucket as the current work). For V1 we wire
 *      the affordance and the layout; populating the list will be
 *      Phase 10's editorial follow-up. Today we render a CTA to the
 *      composer's Works tab.
 *   2. Cross-version comparison — Phase 6 D-022 already exists as a
 *      separate page; the sidebar surfaces a CTA to it.
 *   3. Performers you follow — proxy to Phase 6 favorites; we render
 *      the CTA to the favorites facet for now.
 *
 * Responsive: the parent layout hides the column below 1280px so
 * this file doesn't need media queries.
 */

interface WorkSidebarProps {
  work: Work;
}

export default function WorkSidebar({ work }: WorkSidebarProps) {
  const { navigateToClassicalCompare, navigateToClassicalComposerTab, navigateToClassicalLibrary } =
    useNavigation();

  return (
    <aside
      className="space-y-5"
      aria-label="More about this work"
    >
      {/* Related works */}
      <section className="rounded-xl border border-th-border-subtle/40 bg-th-elevated/40 p-4">
        <h3 className="mb-2 text-[12px] font-bold uppercase tracking-widest text-th-text-secondary">
          Related works
        </h3>
        {work.composerMbid ? (
          <button
            type="button"
            onClick={() =>
              navigateToClassicalComposerTab(
                work.composerMbid!,
                "works",
                work.composerName,
              )
            }
            className="text-left text-[13px] text-th-text-primary/85 hover:text-th-text-primary"
          >
            Browse all works by{" "}
            <span className="font-semibold">
              {work.composerName ?? "this composer"}
            </span>
            <span className="ml-1 text-th-text-muted">→</span>
          </button>
        ) : (
          <p className="text-[12px] text-th-text-muted">
            Composer link unavailable.
          </p>
        )}
      </section>

      {/* Cross-version comparison */}
      <section className="rounded-xl border border-th-border-subtle/40 bg-th-elevated/40 p-4">
        <h3 className="mb-2 text-[12px] font-bold uppercase tracking-widest text-th-text-secondary">
          Compare versions
        </h3>
        {work.recordingCount > 1 ? (
          <button
            type="button"
            onClick={() => navigateToClassicalCompare(work.mbid, work.title)}
            className="text-left text-[13px] text-th-text-primary/85 hover:text-th-text-primary"
          >
            See {work.recordingCount} recordings side-by-side
            <span className="ml-1 text-th-text-muted">→</span>
          </button>
        ) : (
          <p className="text-[12px] text-th-text-muted">
            Only one recording available.
          </p>
        )}
      </section>

      {/* Performers you follow */}
      <section className="rounded-xl border border-th-border-subtle/40 bg-th-elevated/40 p-4">
        <h3 className="mb-2 text-[12px] font-bold uppercase tracking-widest text-th-text-secondary">
          From your library
        </h3>
        <button
          type="button"
          onClick={() => navigateToClassicalLibrary("performers")}
          className="text-left text-[13px] text-th-text-primary/85 hover:text-th-text-primary"
        >
          Performers you follow
          <span className="ml-1 text-th-text-muted">→</span>
        </button>
      </section>
    </aside>
  );
}
