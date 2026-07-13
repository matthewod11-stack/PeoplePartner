/**
 * Prep Brief Modal (FHR-110)
 *
 * Generates and renders an ephemeral pre-meeting brief for one employee:
 * Facts (each cited to a local record) visibly separated from Threads to
 * pull (labeled inference). Nothing is persisted — regenerate is the only
 * "refresh", and closing discards the render (People Map decision 9).
 */

import { useEffect, useReducer } from 'react';
import type { Employee } from '../../lib/types';
import { peopleMapGenerateBrief } from '../../lib/tauri-commands';
import type { PrepBrief } from '../../lib/tauri-commands';
import { Modal } from '../shared';
import {
  initialPrepBriefState,
  isRegenerateSuggested,
  prepBriefReducer,
} from './prepBriefState';

interface PrepBriefModalProps {
  employee: Employee | null;
  onClose: () => void;
}

function CitationChip({ id }: { id: string }) {
  return (
    <span className="inline-block px-1.5 py-0.5 ml-1.5 rounded bg-teal-50 text-teal-700 text-[10px] font-mono align-middle">
      {id}
    </span>
  );
}

function BriefBody({ brief }: { brief: PrepBrief }) {
  return (
    <div className="space-y-5">
      {/* Facts — grounded, cited */}
      <section aria-label="Facts">
        <h4 className="text-xs font-semibold uppercase tracking-wide text-stone-500 mb-2">
          Facts
          <span className="ml-2 normal-case font-normal text-stone-400">
            from this employee&apos;s records
          </span>
        </h4>
        <ul className="space-y-1.5">
          {brief.facts.map((fact, i) => (
            <li key={i} className="text-sm text-stone-700 leading-snug">
              {fact.text}
              <CitationChip id={fact.citationId} />
            </li>
          ))}
        </ul>
      </section>

      {/* Threads — inference, visibly separated */}
      <section
        aria-label="Threads to pull"
        className="border-t border-stone-200/60 pt-4"
      >
        <h4 className="text-xs font-semibold uppercase tracking-wide text-amber-600 mb-2">
          ⚠️ Threads to pull — prep suggestions, not facts
        </h4>
        {brief.threads.length > 0 ? (
          <ul className="space-y-3">
            {brief.threads.map((thread, i) => (
              <li key={i} className="text-sm leading-snug">
                <p className="text-stone-400 text-xs">
                  Anchor: {thread.anchorFact}
                  <CitationChip id={thread.anchorCitationId} />
                </p>
                <p className="text-stone-700 mt-0.5">{thread.question}</p>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-stone-500 italic">
            {brief.thinRecordNote ??
              'No threads could be anchored to this record.'}
          </p>
        )}
      </section>

      {/* Thin-record note when threads exist is impossible by construction,
          but a facts-only brief always explains itself above. */}
      <p className="text-[11px] text-stone-400">
        Briefs are generated on demand and never stored — only the audit-log
        entry is kept. Regenerate any time.
      </p>
    </div>
  );
}

export function PrepBriefModal({ employee, onClose }: PrepBriefModalProps) {
  const [state, dispatch] = useReducer(prepBriefReducer, initialPrepBriefState);

  const employeeId = employee?.id ?? null;

  // Auto-generate when the modal opens for an employee.
  useEffect(() => {
    if (!employeeId) {
      dispatch({ type: 'closed' });
      return;
    }
    let stale = false;
    dispatch({ type: 'generate' });
    peopleMapGenerateBrief(employeeId)
      .then((brief) => {
        if (!stale) dispatch({ type: 'succeeded', brief });
      })
      .catch((err: unknown) => {
        if (!stale) dispatch({ type: 'failed', message: String(err) });
      });
    return () => {
      stale = true;
    };
  }, [employeeId]);

  const regenerate = () => {
    if (!employeeId) return;
    dispatch({ type: 'generate' });
    peopleMapGenerateBrief(employeeId)
      .then((brief) => dispatch({ type: 'succeeded', brief }))
      .catch((err: unknown) =>
        dispatch({ type: 'failed', message: String(err) })
      );
  };

  const handleClose = () => {
    dispatch({ type: 'closed' });
    onClose();
  };

  return (
    <Modal
      isOpen={!!employee}
      onClose={handleClose}
      title={employee ? `Prep Brief — ${employee.full_name}` : 'Prep Brief'}
    >
      <div className="min-h-[10rem]">
        {state.kind === 'loading' && (
          <div
            className="flex flex-col items-center justify-center py-10 text-stone-500"
            role="status"
          >
            <div className="w-6 h-6 border-2 border-teal-500 border-t-transparent rounded-full animate-spin mb-3" />
            <p className="text-sm">Assembling brief from local records…</p>
          </div>
        )}

        {state.kind === 'error' && (
          <div className="py-6 text-center" role="alert">
            <p className="text-sm text-stone-700 mb-1">
              {isRegenerateSuggested(state.message)
                ? "This generation didn't ground itself in the record."
                : 'Brief generation failed.'}
            </p>
            <p className="text-xs text-stone-500 mb-4 break-words">
              {state.message}
            </p>
            <button
              onClick={regenerate}
              className="px-3 py-1.5 text-sm rounded-lg bg-teal-600 text-white hover:bg-teal-700 transition-colors"
            >
              Try again
            </button>
          </div>
        )}

        {state.kind === 'ready' && (
          <>
            <BriefBody brief={state.brief} />
            <div className="flex justify-end gap-2 mt-5 pt-3 border-t border-stone-200/60">
              <button
                onClick={regenerate}
                className="px-3 py-1.5 text-sm rounded-lg text-stone-600 hover:bg-stone-100 transition-colors"
              >
                Regenerate
              </button>
              <button
                onClick={handleClose}
                className="px-3 py-1.5 text-sm rounded-lg bg-stone-800 text-white hover:bg-stone-700 transition-colors"
              >
                Done
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
