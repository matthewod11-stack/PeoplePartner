// People Partner — Recruit module (talent sourcing)
//
// FHR-86 (S4.2): the intake conversation. Drives the headless intake graph via
// state-passing commands, produces a runnable SearchConfig, and fires the full
// Sourcerer pipeline via recruiting_run_search (FHR-73 S1.1 Task 7). Reuses
// the HR Chat view components for continuity between the two modules.

import { useState, useRef } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { MessageList, ChatInput } from '../chat';
import {
  recruitingIntakeStart,
  recruitingIntakeStartFromSeed,
  recruitingIntakeStep,
  recruitingIntakeExtract,
  recruitingRunSearch,
} from '../../lib/tauri-commands';
import type { RunSearchResult, CandidatePreview } from '../../lib/tauri-commands';
import type {
  Message,
  IntakeCommandError,
  RecruitingSearchError,
  SearchConfig,
} from '../../lib/types';

type ViewState =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | { kind: 'conversing'; messages: Message[]; state: string; stepping: boolean; stepError?: string }
  | { kind: 'extracting'; messages: Message[] }
  | { kind: 'searching'; config: SearchConfig }
  | { kind: 'results'; config: SearchConfig; data: RunSearchResult }
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

function isRecruitingSearchError(e: unknown): e is RecruitingSearchError {
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

  async function handleAttachSeed() {
    if (pendingRef.current) return;
    pendingRef.current = true;
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'JD or transcript', extensions: ['md', 'txt', 'pdf', 'docx'] }],
      });
      if (typeof path !== 'string') return; // cancelled (null) or unexpected array
      setView({ kind: 'starting' });
      const turn = await recruitingIntakeStartFromSeed({ filePath: path });
      const messages = turn.prompt ? [newMessage('assistant', turn.prompt)] : [];
      setView({ kind: 'conversing', messages, state: turn.state, stepping: false });
    } catch (e) {
      routeIntakeError(e);
    } finally {
      pendingRef.current = false;
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
    // Guard: need at least one query tier to run the pipeline.
    if (!config.tiers[0]?.queries[0]?.text) {
      setView({ kind: 'configNoQuery', config });
      return;
    }
    setView({ kind: 'searching', config });
    try {
      const data = await recruitingRunSearch(config);
      setView({ kind: 'results', config, data });
    } catch (raw) {
      const error: RecruitingSearchError = isRecruitingSearchError(raw)
        ? raw
        : { kind: 'Internal', message: String(raw) };
      // MissingKey / InvalidKey → route to the key banner.
      if (error.kind === 'MissingKey' || error.kind === 'InvalidKey') {
        setView({ kind: 'missingKey', which: 'exa' });
      } else {
        setView({ kind: 'searchError', config, error });
      }
    }
  }

  return (
    <div className="h-full flex flex-col">
      {view.kind === 'idle' && <EmptyState onStart={startIntake} onAttach={handleAttachSeed} />}
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

function EmptyState({ onStart, onAttach }: { onStart: () => void; onAttach: () => void }) {
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
      <div className="flex flex-col items-center gap-2">
        <button
          type="button"
          onClick={onStart}
          className="px-4 py-2 bg-primary-600 text-white text-sm font-medium rounded-md hover:bg-primary-700 transition-colors"
        >
          Start intake
        </button>
        <button
          type="button"
          onClick={onAttach}
          className="px-4 py-2 text-sm font-medium text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-md transition-colors"
        >
          Attach JD or transcript
        </button>
      </div>
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
    case 'FeatureDisabled':
      // Only reachable when the TS and Rust RECRUITING_ENABLED flags disagree
      // (#109) — the UI is visible but the IPC surface is compiled off.
      heading = 'Recruiting is disabled in this build';
      message =
        'The Rust-side gate is off. Flip RECRUITING_ENABLED in both ' +
        'src/lib/featureFlags.ts and src-tauri/src/commands/recruiting.rs, then rebuild.';
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

/** Renders the full pipeline result (candidates + cost summary). */
function ResultsList({ data }: { data: RunSearchResult }) {
  const { candidates, discoveryStats, costReport } = data;

  if (candidates.length === 0) {
    return (
      <div className="px-6 py-8">
        <p className="text-sm text-stone-500">No candidates found for this search.</p>
        <p className="mt-1 text-xs text-stone-400">
          {discoveryStats.queryFailures > 0 && `${discoveryStats.queryFailures} queries failed. `}
          {discoveryStats.stoppedEarly && 'Search stopped early (budget). '}
        </p>
      </div>
    );
  }

  return (
    <div className="px-6 py-4">
      <p className="mb-3 text-xs text-stone-400">
        {candidates.length} candidate{candidates.length !== 1 ? 's' : ''} found
        {discoveryStats.stoppedEarly && ' (partial — budget reached)'}
        {' · '}${costReport.snapshot.totalUsd.toFixed(4)} spent
      </p>
      <ul className="space-y-2">
        {candidates.map((c) => (
          <CandidateCard key={c.id} candidate={c} />
        ))}
      </ul>
    </div>
  );
}

function CandidateCard({ candidate }: { candidate: CandidatePreview }) {
  const { id, name } = candidate;

  // Primary URL = first URL from the first source that has one.
  let primaryUrl: string | undefined;
  for (const src of Object.values(candidate.sources)) {
    if (src.urls.length > 0) {
      primaryUrl = src.urls[0];
      break;
    }
  }

  return (
    <li className="rounded-lg border border-stone-200 bg-white p-4 hover:border-stone-300 transition-colors">
      {primaryUrl ? (
        <a href={primaryUrl} target="_blank" rel="noopener noreferrer" className="block group">
          <h3 className="text-sm font-medium text-primary-700 group-hover:text-primary-800 group-hover:underline">
            {name}
          </h3>
          <p className="mt-0.5 text-xs text-stone-400 truncate">{primaryUrl}</p>
        </a>
      ) : (
        <div>
          <h3 className="text-sm font-medium text-stone-800">{name}</h3>
          <p className="mt-0.5 text-xs text-stone-400 italic">no URL</p>
        </div>
      )}
      <p className="mt-1 text-xs text-stone-400 font-mono truncate">{id}</p>
    </li>
  );
}

export default RecruitingView;
