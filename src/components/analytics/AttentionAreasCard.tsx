/**
 * AttentionAreasCard Component (V2.4.1)
 *
 * Displays team-level attention signals based on tenure, performance,
 * and engagement heuristics. Always includes a disclaimer banner.
 *
 * Key guardrails:
 * - Team-level only (never individual predictions)
 * - Strong disclaimers on all outputs
 * - Factor transparency (show what contributed)
 */

import { useState, useEffect, useCallback } from 'react';
import { getAttentionSignals, isSignalsEnabled } from '../../lib/tauri-commands';
import type { TeamAttentionSignal, AttentionAreasSummary } from '../../lib/signals-types';
import {
  getTopFactor,
  getAttentionBadgeColor,
  formatAttentionScore,
} from '../../lib/signals-types';
import { TeamThemeModal } from './TeamThemeModal';

interface AttentionAreasCardProps {
  /** Optional: refresh trigger (increment to refetch) */
  refreshKey?: number;
}

export function AttentionAreasCard({ refreshKey = 0 }: AttentionAreasCardProps) {
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [data, setData] = useState<AttentionAreasSummary | null>(null);
  const [selectedTeam, setSelectedTeam] = useState<TeamAttentionSignal | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      // Check if feature is enabled first
      const isEnabled = await isSignalsEnabled();
      setEnabled(isEnabled);

      if (!isEnabled) {
        setLoading(false);
        return;
      }

      // Fetch the signals data
      const signals = await getAttentionSignals();
      setData(signals);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load signals';
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData, refreshKey]);

  // Don't render anything if feature is disabled
  if (!enabled && !loading) {
    return null;
  }

  // Loading state
  if (loading) {
    return (
      <div className="p-4 bg-white border border-stone-200 rounded-xl">
        <div className="animate-pulse space-y-3">
          <div className="h-4 bg-stone-200 rounded w-1/3" />
          <div className="h-10 bg-stone-100 rounded" />
          <div className="h-6 bg-stone-100 rounded" />
          <div className="h-6 bg-stone-100 rounded" />
        </div>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="p-4 bg-red-50 border border-red-200 rounded-xl">
        <div className="flex items-center gap-2 text-red-700">
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span className="text-sm font-medium">Error loading attention signals</span>
        </div>
        <p className="mt-1 text-xs text-red-600">{error}</p>
      </div>
    );
  }

  // Empty state
  if (!data || data.teams.length === 0) {
    return (
      <div className="p-4 bg-white border border-stone-200 rounded-xl">
        <h3 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-3">
          <svg className="w-4 h-4 text-amber-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          Team Attention Areas
        </h3>
        <p className="text-sm text-stone-500">
          No data available. Teams need at least 5 members with performance and engagement data.
        </p>
      </div>
    );
  }

  return (
    <>
      <div className="p-4 bg-white border border-stone-200 rounded-xl">
        {/* Header */}
        <h3 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-3">
          <svg className="w-4 h-4 text-amber-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          Team Attention Areas
        </h3>

        {/* Disclaimer Banner - Always visible */}
        <div className="mb-4 p-3 bg-amber-50 border border-amber-200 rounded-lg">
          <div className="flex gap-2">
            <svg className="w-4 h-4 text-amber-600 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p className="text-xs text-amber-800">
              {data.disclaimer}
            </p>
          </div>
        </div>

        {/* Team List */}
        <div className="space-y-2">
          {data.teams.map((signal) => (
            <TeamRow
              key={signal.team}
              signal={signal}
              onClick={() => setSelectedTeam(signal)}
            />
          ))}
        </div>
      </div>

      {/* Theme Modal */}
      {selectedTeam && (
        <TeamThemeModal
          signal={selectedTeam}
          isOpen={true}
          onClose={() => setSelectedTeam(null)}
        />
      )}
    </>
  );
}

interface TeamRowProps {
  signal: TeamAttentionSignal;
  onClick: () => void;
}

function TeamRow({ signal, onClick }: TeamRowProps) {
  const topFactor = getTopFactor(signal);
  const badgeColor = getAttentionBadgeColor(signal.attention_level);

  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full text-left p-3 rounded-lg border border-stone-100 hover:border-stone-200 hover:bg-stone-50 transition-colors"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-stone-800 truncate">
              {signal.team}
            </span>
            <span className="text-xs text-stone-500">
              ({signal.headcount})
            </span>
          </div>
          <p className="text-xs text-stone-500 mt-0.5 truncate">
            Top factor: {topFactor.value}
          </p>
        </div>

        {/* Attention Score Badge */}
        <span
          className={`
            flex-shrink-0 px-2 py-1 text-xs font-medium rounded border
            ${badgeColor}
          `}
        >
          {signal.attention_level.charAt(0).toUpperCase() + signal.attention_level.slice(1)}: {formatAttentionScore(signal.attention_score)}
        </span>
      </div>
    </button>
  );
}

export default AttentionAreasCard;
