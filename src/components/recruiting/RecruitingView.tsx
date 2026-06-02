// People Partner — Recruit module (talent sourcing)
//
// FHR-86 (S4.2): the intake conversation. Drives the headless intake graph via
// state-passing commands, produces a runnable SearchConfig, and fires a real
// Exa search (bridged through the existing recruiting_search_exa until the
// tiered executor lands in S1.1). Reuses the HR Chat view components for
// continuity between the two modules.

import { useState, useRef } from 'react';
import { MessageList, ChatInput } from '../chat';
import {
  recruitingIntakeStart,
  recruitingIntakeStep,
  recruitingIntakeExtract,
  recruitingSearchExa,
} from '../../lib/tauri-commands';
import type {
  Message,
  IntakeCommandError,
  ExaHit,
  ExaSearchResponse,
  RecruitingSearchError,
  SearchConfig,
} from '../../lib/types';

type ViewState =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | { kind: 'conversing'; messages: Message[]; state: string; stepping: boolean; stepError?: string }
  | { kind: 'extracting'; messages: Message[] }
  | { kind: 'searching'; config: SearchConfig }
  | { kind: 'results'; config: SearchConfig; data: ExaSearchResponse }
  | { kind: 'configNoQuery'; config: SearchConfig }
  | { kind: 'missingKey'; which: 'llm' | 'exa' }
  | { kind: 'error'; message: string }
  | { kind: 'searchError'; config: SearchConfig; error: RecruitingSearchError };

interface RecruitingViewProps {
  onOpenSettings?: () => void;
}

function newMessage(role: 'user' | 'assistant', content: string): Message {
  return { id: crypto.randomUUID(), role, content, timestamp: new Date().toISOString() };
}

function isIntakeError(e: unknown): e is IntakeCommandError {
  return typeof e === 'object' && e !== null && 'kind' in e;
}

export function RecruitingView({ onOpenSettings }: RecruitingViewProps = {}) {
  const [view, setView] = useState<ViewState>({ kind: 'idle' });
  const pendingRef = useRef(false);

  function routeIntakeError(e: unknown) {
    if (isIntakeError(e) && e.kind === 'MissingLlmKey') return setView({ kind: 'missingKey', which: 'llm' });
    if (isIntakeError(e) && e.kind === 'MissingExaKey') return setView({ kind: 'missingKey', which: 'exa' });
    const message = isIntakeError(e) && 'message' in e ? e.message : String(e);
    setView({ kind: 'error', message });
  }

  async function startIntake() {
    setView({ kind: 'starting' });
    try {
      const turn = await recruitingIntakeStart();
      const messages = turn.prompt ? [newMessage('assistant', turn.prompt)] : [];
      setView({ kind: 'conversing', messages, state: turn.state, stepping: false });
    } catch (e) {
      routeIntakeError(e);
    }
  }

  async function submitTurn(response: string) {
    if (view.kind !== 'conversing' || view.stepping || pendingRef.current) return;
    pendingRef.current = true;
    try {
      const priorState = view.state;
      const withUser = [...view.messages, newMessage('user', response)];
      setView({ kind: 'conversing', messages: withUser, state: priorState, stepping: true });
      try {
        const turn = await recruitingIntakeStep(priorState, response);
        const next = turn.prompt ? [...withUser, newMessage('assistant', turn.prompt)] : withUser;
        if (turn.done) {
          setView({ kind: 'extracting', messages: next });
          await extractAndSearch(turn.state);
        } else {
          setView({ kind: 'conversing', messages: next, state: turn.state, stepping: false });
        }
      } catch (e) {
        if (isIntakeError(e) && (e.kind === 'MissingLlmKey' || e.kind === 'MissingExaKey')) {
          routeIntakeError(e);
          return;
        }
        const message = isIntakeError(e) && 'message' in e ? e.message : String(e);
        setView({ kind: 'conversing', messages: withUser, state: priorState, stepping: false, stepError: message });
      }
    } finally {
      pendingRef.current = false;
    }
  }

  async function extractAndSearch(state: string) {
    let config: SearchConfig;
    try {
      const result = await recruitingIntakeExtract(state);
      config = result.searchConfig;
    } catch (e) {
      return routeIntakeError(e);
    }
    const topQuery = config.tiers[0]?.queries[0]?.text;
    if (!topQuery) {
      setView({ kind: 'configNoQuery', config });
      return;
    }
    setView({ kind: 'searching', config });
    const search = await recruitingSearchExa(topQuery);
    setView(
      search.ok
        ? { kind: 'results', config, data: search.data }
        : { kind: 'searchError', config, error: search.error },
    );
  }

  return (
    <div className="h-full flex flex-col">
      {view.kind === 'idle' && <EmptyState onStart={startIntake} />}
      {view.kind === 'starting' && <CenterNote text="Starting intake…" />}

      {view.kind === 'conversing' && (
        <>
          <div className="flex-1 overflow-y-auto">
            <MessageList messages={view.messages} isLoading={view.stepping} />
          </div>
          {view.stepError && (
            <div className="px-6 py-2 text-xs text-red-700 bg-red-50 border-t border-red-100" role="alert">
              Couldn't process that: {view.stepError}. Try sending it again.
            </div>
          )}
          <ChatInput onSubmit={submitTurn} disabled={view.stepping} placeholder="Answer to continue…" />
        </>
      )}

      {view.kind === 'extracting' && (
        <>
          <div className="flex-1 overflow-y-auto">
            <MessageList messages={view.messages} isLoading />
          </div>
          <CenterNote text="Building your search configuration…" />
        </>
      )}

      {view.kind === 'searching' && (
        <ConfigHeaderWithBody config={view.config}>
          <CenterNote text="Running search…" />
        </ConfigHeaderWithBody>
      )}

      {view.kind === 'results' && (
        <ConfigHeaderWithBody config={view.config}>
          <div className="flex-1 overflow-y-auto">
            <ResultsList data={view.data} />
          </div>
        </ConfigHeaderWithBody>
      )}

      {view.kind === 'configNoQuery' && (
        <ConfigHeaderWithBody config={view.config}>
          <div className="px-6 py-8">
            <p className="text-sm text-stone-500">
              Configuration generated, but it has no runnable query yet. The full
              tiered executor lands in a later milestone.
            </p>
          </div>
        </ConfigHeaderWithBody>
      )}

      {view.kind === 'searchError' && (
        <ConfigHeaderWithBody config={view.config}>
          <ErrorState error={view.error} />
        </ConfigHeaderWithBody>
      )}

      {view.kind === 'missingKey' && (
        <KeyBanner which={view.which} onOpenSettings={onOpenSettings} />
      )}

      {view.kind === 'error' && (
        <div className="px-6 py-8">
          <div className="rounded-lg border border-red-200 bg-red-50 p-4 max-w-xl">
            <h3 className="text-sm font-semibold text-red-900">Intake error</h3>
            <p className="mt-1 text-sm text-red-800 font-mono break-words">{view.message}</p>
            <button
              type="button"
              onClick={startIntake}
              className="mt-3 inline-flex items-center px-3 py-1.5 text-xs font-medium text-red-900 bg-red-100 hover:bg-red-200 rounded-md transition-colors"
            >
              Start over
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Sub-components
// ============================================================================

function EmptyState({ onStart }: { onStart: () => void }) {
  return (
    <div className="h-full flex flex-col items-center justify-center text-center px-6 py-16">
      <div className="w-14 h-14 rounded-full bg-primary-100 flex items-center justify-center mb-4">
        <svg className="w-7 h-7 text-primary-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5} aria-hidden="true">
          <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
        </svg>
      </div>
      <h2 className="text-lg font-display font-semibold text-stone-800 mb-1">Recruit</h2>
      <p className="text-sm text-stone-500 max-w-sm mb-5">
        A short guided intake turns a role into a precise candidate search — seeded
        by the team you're hiring for. Answer a few questions and we'll run it.
      </p>
      <button
        type="button"
        onClick={onStart}
        className="px-4 py-2 bg-primary-600 text-white text-sm font-medium rounded-md hover:bg-primary-700 transition-colors"
      >
        Start intake
      </button>
    </div>
  );
}

function CenterNote({ text }: { text: string }) {
  return <div className="px-6 py-6 shrink-0" role="status" aria-live="polite"><p className="text-sm text-stone-500">{text}</p></div>;
}

function ConfigHeaderWithBody({ config, children }: { config: SearchConfig; children: React.ReactNode }) {
  const queryCount = config.tiers.reduce((n, t) => n + t.queries.length, 0);
  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-stone-200 px-6 py-3 bg-stone-50">
        <h3 className="text-sm font-semibold text-stone-800">{config.roleName}</h3>
        <p className="mt-0.5 text-xs text-stone-500">
          {config.tiers.length} tiers · {queryCount} queries · {config.antiFilters.length} anti-filters · up to {config.maxCandidates} candidates · ${config.maxCostUsd} cap
        </p>
      </div>
      {children}
    </div>
  );
}

function KeyBanner({ which, onOpenSettings }: { which: 'llm' | 'exa'; onOpenSettings?: () => void }) {
  const heading = which === 'exa' ? 'Recruiting needs your Exa API key' : 'Recruiting needs an AI provider key';
  const detail =
    which === 'exa'
      ? 'The intake search uses Exa to find candidates. Add your Exa key in Settings to run it.'
      : 'The intake uses your AI provider to structure the conversation. Add a provider key in Settings to start.';
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
            Open Settings →
          </button>
        )}
      </div>
    </div>
  );
}

function ErrorState({ error }: { error: RecruitingSearchError }) {
  // `error.kind` discriminates the union — TS narrows each case, no casts needed.
  let heading: string;
  let message: string;
  switch (error.kind) {
    case 'MissingKey':
    case 'InvalidKey':
      heading = 'Exa key problem';
      message = 'Add or update your Exa key in Settings.';
      break;
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
        <p className="mt-1 text-sm text-red-800 font-mono break-words">{message}</p>
      </div>
    </div>
  );
}

function ResultsList({ data }: { data: ExaSearchResponse }) {
  if (data.results.length === 0) {
    return <div className="px-6 py-8"><p className="text-sm text-stone-500">No results for the generated query.</p></div>;
  }
  return (
    <div className="px-6 py-4">
      {data.autopromptString && (
        <p className="mb-3 text-xs text-stone-400">
          Exa rewrote the query: <span className="italic">&ldquo;{data.autopromptString}&rdquo;</span>
        </p>
      )}
      <ul className="space-y-3">
        {data.results.map((hit) => <HitCard key={hit.id} hit={hit} />)}
      </ul>
    </div>
  );
}

function HitCard({ hit }: { hit: ExaHit }) {
  return (
    <li className="rounded-lg border border-stone-200 bg-white p-4 hover:border-stone-300 transition-colors">
      <a href={hit.url} target="_blank" rel="noopener noreferrer" className="block group">
        <h3 className="text-sm font-medium text-primary-700 group-hover:text-primary-800 group-hover:underline">
          {hit.title || hit.url}
        </h3>
        <p className="mt-0.5 text-xs text-stone-400 truncate">{hit.url}</p>
      </a>
      <div className="mt-2 flex flex-wrap gap-3 text-xs text-stone-500">
        {hit.author && <span>by {hit.author}</span>}
        {hit.publishedDate && <span>{hit.publishedDate}</span>}
        {typeof hit.score === 'number' && <span className="text-stone-400">score {hit.score.toFixed(2)}</span>}
      </div>
      {hit.summary && <p className="mt-2 text-sm text-stone-600">{hit.summary}</p>}
    </li>
  );
}

export default RecruitingView;
