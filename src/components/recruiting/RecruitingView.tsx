// People Partner — Recruit module (talent sourcing)
//
// FHR-72 (S0.3): round-trip one live Exa search to the UI.
//   - Single-line input → submit → render raw Exa hits as cards.
//   - BYOK: the Tauri command reads the Exa key from macOS Keychain.
//   - Missing/invalid-key path renders the inline banner; the call-site is
//     responsible for branching on the result's `error.kind`.

import { useState } from 'react';
import { recruitingSearchExa } from '../../lib/tauri-commands';
import type {
  ExaHit,
  ExaSearchResponse,
  RecruitingSearchError,
} from '../../lib/types';

type ViewState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'success'; data: ExaSearchResponse }
  | { kind: 'error'; error: RecruitingSearchError };

interface RecruitingViewProps {
  /** Open the Settings panel — passed in by App.tsx so the missing-key
   *  banner can deep-link to the Recruiting section. Optional so the
   *  component remains usable in isolated previews/stories. */
  onOpenSettings?: () => void;
}

export function RecruitingView({ onOpenSettings }: RecruitingViewProps = {}) {
  const [query, setQuery] = useState('');
  const [view, setView] = useState<ViewState>({ kind: 'idle' });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = query.trim();
    if (!trimmed || view.kind === 'loading') return;
    setView({ kind: 'loading' });
    const result = await recruitingSearchExa(trimmed);
    setView(
      result.ok
        ? { kind: 'success', data: result.data }
        : { kind: 'error', error: result.error },
    );
  };

  // Render helper — discriminated narrowing reads cleaner as a function than
  // a chain of ternaries, and lets TS narrow `view.error.kind` in the
  // missing-key branch without explicit casts.
  function renderContent() {
    if (view.kind === 'idle') return <EmptyState />;
    if (view.kind === 'loading') return <LoadingState />;
    if (view.kind === 'success') return <ResultsList data={view.data} />;
    // view.kind === 'error'
    if (view.error.kind === 'MissingKey' || view.error.kind === 'InvalidKey') {
      return (
        <MissingKeyBanner
          kind={view.error.kind}
          onOpenSettings={onOpenSettings}
        />
      );
    }
    return <ErrorState error={view.error} />;
  }

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-stone-200 px-6 py-4">
        <form onSubmit={handleSubmit} className="flex gap-2">
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search for candidates…"
            className="flex-1 px-3 py-2 border border-stone-300 rounded-md text-sm placeholder-stone-400 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-primary-500"
            disabled={view.kind === 'loading'}
            autoFocus
            aria-label="Recruiting search query"
          />
          <button
            type="submit"
            disabled={view.kind === 'loading' || !query.trim()}
            className="px-4 py-2 bg-primary-600 text-white text-sm font-medium rounded-md hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {view.kind === 'loading' ? 'Searching…' : 'Search'}
          </button>
        </form>
      </div>

      <div className="flex-1 overflow-y-auto">{renderContent()}</div>
    </div>
  );
}

// ============================================================================
// Sub-components
// ============================================================================

function MissingKeyBanner({
  kind,
  onOpenSettings,
}: {
  kind: 'MissingKey' | 'InvalidKey';
  onOpenSettings?: () => void;
}) {
  const heading =
    kind === 'MissingKey'
      ? 'Recruiting needs your Exa API key'
      : 'Your Exa API key was rejected';
  const detail =
    kind === 'MissingKey'
      ? 'Recruiting search uses Exa to discover candidates. Add your key in Settings to start searching.'
      : 'Exa returned 401 for the stored key. Update it in Settings and try again.';

  return (
    <div className="px-6 py-8">
      <div className="rounded-lg border border-amber-200 bg-amber-50 p-5 max-w-xl">
        <h3 className="text-sm font-semibold text-amber-900">{heading}</h3>
        <p className="mt-1 text-sm text-amber-800">{detail}</p>
        {onOpenSettings && (
          <button
            type="button"
            onClick={onOpenSettings}
            className="mt-3 inline-flex items-center px-3 py-1.5 text-xs font-medium text-amber-900 bg-amber-100 hover:bg-amber-200 rounded-md transition-colors"
          >
            Add your Exa key in Settings →
          </button>
        )}
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="h-full flex flex-col items-center justify-center text-center px-6 py-16">
      <div className="w-14 h-14 rounded-full bg-primary-100 flex items-center justify-center mb-4">
        <svg
          className="w-7 h-7 text-primary-600"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
          />
        </svg>
      </div>
      <h2 className="text-lg font-display font-semibold text-stone-800 mb-1">
        Recruit
      </h2>
      <p className="text-sm text-stone-500 max-w-sm">
        Context-aware talent sourcing, seeded by the employee data you already
        have. Type a query above to start.
      </p>
    </div>
  );
}

function LoadingState() {
  return (
    <div className="px-6 py-8">
      <p className="text-sm text-stone-500">Searching Exa…</p>
    </div>
  );
}

// Narrowed: this only receives the soft-error variants. Missing/invalid key
// is handled separately by `MissingKeyBanner`.
type SoftError = Exclude<
  RecruitingSearchError,
  { kind: 'MissingKey' } | { kind: 'InvalidKey' }
>;

function ErrorState({ error }: { error: SoftError }) {
  let heading: string;
  let message: string;
  switch (error.kind) {
    case 'RateLimit':
      heading = 'Exa rate limit hit';
      message = error.message;
      break;
    case 'Network':
      heading = "Couldn't reach Exa";
      message = error.message;
      break;
    case 'ExaApi':
      heading = 'Exa returned an error';
      message = `${error.status}: ${error.body}`;
      break;
    case 'Internal':
      heading = 'Unexpected error';
      message = error.message;
      break;
  }

  return (
    <div className="px-6 py-8">
      <div className="rounded-lg border border-red-200 bg-red-50 p-4 max-w-xl">
        <h3 className="text-sm font-semibold text-red-900">{heading}</h3>
        <p className="mt-1 text-sm text-red-800 font-mono break-words">
          {message}
        </p>
      </div>
    </div>
  );
}

function ResultsList({ data }: { data: ExaSearchResponse }) {
  if (data.results.length === 0) {
    return (
      <div className="px-6 py-8">
        <p className="text-sm text-stone-500">No results for that query.</p>
      </div>
    );
  }

  return (
    <div className="px-6 py-4">
      {data.autopromptString && (
        <p className="mb-3 text-xs text-stone-400">
          Exa rewrote your query:{' '}
          <span className="italic">&ldquo;{data.autopromptString}&rdquo;</span>
        </p>
      )}
      <ul className="space-y-3">
        {data.results.map((hit) => (
          <HitCard key={hit.id} hit={hit} />
        ))}
      </ul>
    </div>
  );
}

function HitCard({ hit }: { hit: ExaHit }) {
  return (
    <li className="rounded-lg border border-stone-200 bg-white p-4 hover:border-stone-300 transition-colors">
      <a
        href={hit.url}
        target="_blank"
        rel="noopener noreferrer"
        className="block group"
      >
        <h3 className="text-sm font-medium text-primary-700 group-hover:text-primary-800 group-hover:underline">
          {hit.title || hit.url}
        </h3>
        <p className="mt-0.5 text-xs text-stone-400 truncate">{hit.url}</p>
      </a>
      <div className="mt-2 flex flex-wrap gap-3 text-xs text-stone-500">
        {hit.author && <span>by {hit.author}</span>}
        {hit.publishedDate && <span>{hit.publishedDate}</span>}
        {typeof hit.score === 'number' && (
          <span className="text-stone-400">score {hit.score.toFixed(2)}</span>
        )}
      </div>
      {hit.summary && (
        <p className="mt-2 text-sm text-stone-600">{hit.summary}</p>
      )}
    </li>
  );
}

export default RecruitingView;
