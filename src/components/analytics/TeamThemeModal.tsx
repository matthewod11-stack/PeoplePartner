/**
 * TeamThemeModal Component (V2.4.1)
 *
 * Shows detailed factor breakdown and common themes for a team's
 * attention signal. No individual employee names are shown.
 */

import { Modal } from '../shared/Modal';
import type { TeamAttentionSignal, ThemeOccurrence } from '../../lib/signals-types';
import { getAttentionBadgeColor, formatAttentionScore } from '../../lib/signals-types';

interface TeamThemeModalProps {
  /** The team signal to display */
  signal: TeamAttentionSignal;
  /** Whether the modal is open */
  isOpen: boolean;
  /** Called when the modal should close */
  onClose: () => void;
}

export function TeamThemeModal({ signal, isOpen, onClose }: TeamThemeModalProps) {
  const badgeColor = getAttentionBadgeColor(signal.attention_level);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={`${signal.team} Team Details`}
      maxWidth="max-w-md"
    >
      <div className="space-y-5">
        {/* Header with score */}
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-stone-500">
              {signal.headcount} active employees
            </p>
          </div>
          <span
            className={`
              px-3 py-1.5 text-sm font-medium rounded-lg border
              ${badgeColor}
            `}
          >
            Score: {formatAttentionScore(signal.attention_score)}
          </span>
        </div>

        {/* Disclaimer */}
        <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg">
          <p className="text-xs text-amber-800">
            These are heuristic indicators based on aggregate team patterns, not predictions
            about individuals. Use as conversation starters, not conclusions.
          </p>
        </div>

        {/* Factor Breakdown */}
        <div>
          <h4 className="text-sm font-medium text-stone-700 mb-3">
            Factor Breakdown
          </h4>
          <div className="space-y-3">
            <FactorRow
              name="Tenure"
              score={signal.tenure_factor.score}
              details={[
                { label: 'Under 1 year', value: `${Math.round(signal.tenure_factor.pct_under_1yr)}%` },
                { label: '3-5 years', value: `${Math.round(signal.tenure_factor.pct_3_to_5yr)}%` },
              ]}
            />
            <FactorRow
              name="Performance"
              score={signal.performance_factor.score}
              details={[
                { label: 'Declining ratings', value: `${Math.round(signal.performance_factor.pct_declining)}%` },
                { label: 'Needs improvement', value: `${Math.round(signal.performance_factor.pct_needs_improvement)}%` },
              ]}
            />
            <FactorRow
              name="Engagement"
              score={signal.engagement_factor.score}
              details={[
                { label: 'Detractors (eNPS 0-6)', value: `${Math.round(signal.engagement_factor.pct_detractors)}%` },
                { label: 'Passives (eNPS 7-8)', value: `${Math.round(signal.engagement_factor.pct_passives)}%` },
              ]}
            />
          </div>
        </div>

        {/* Common Themes */}
        {signal.common_themes.length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-stone-700 mb-3">
              Common Review Themes
            </h4>
            <div className="flex flex-wrap gap-2">
              {signal.common_themes.map((theme) => (
                <ThemeBadge key={theme.theme} theme={theme} />
              ))}
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}

interface FactorRowProps {
  name: string;
  score: number;
  details: { label: string; value: string }[];
}

function FactorRow({ name, score, details }: FactorRowProps) {
  // Determine bar color based on score
  const getBarColor = (score: number) => {
    if (score >= 70) return 'bg-red-500';
    if (score >= 50) return 'bg-amber-500';
    if (score >= 30) return 'bg-stone-400';
    return 'bg-green-500';
  };

  return (
    <div className="p-3 bg-stone-50 rounded-lg">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-stone-700">{name}</span>
        <span className="text-xs text-stone-500">{Math.round(score)} / 100</span>
      </div>

      {/* Score bar */}
      <div className="h-2 bg-stone-200 rounded-full overflow-hidden mb-2">
        <div
          className={`h-full rounded-full transition-all ${getBarColor(score)}`}
          style={{ width: `${Math.min(score, 100)}%` }}
        />
      </div>

      {/* Details */}
      <div className="flex gap-4 text-xs text-stone-600">
        {details.map((detail) => (
          <span key={detail.label}>
            {detail.label}: <span className="font-medium">{detail.value}</span>
          </span>
        ))}
      </div>
    </div>
  );
}

interface ThemeBadgeProps {
  theme: ThemeOccurrence;
}

function ThemeBadge({ theme }: ThemeBadgeProps) {
  // Sentiment colors
  const getSentimentColor = (sentiment: string) => {
    switch (sentiment) {
      case 'positive':
        return 'bg-green-50 text-green-700 border-green-200';
      case 'negative':
        return 'bg-red-50 text-red-700 border-red-200';
      case 'mixed':
        return 'bg-amber-50 text-amber-700 border-amber-200';
      default:
        return 'bg-stone-50 text-stone-700 border-stone-200';
    }
  };

  // Format theme name for display (e.g., "technical-growth" -> "Technical Growth")
  const formatTheme = (theme: string) => {
    return theme
      .split('-')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  };

  return (
    <span
      className={`
        inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-xs font-medium
        ${getSentimentColor(theme.sentiment)}
      `}
    >
      {formatTheme(theme.theme)}
      <span className="text-stone-400">({theme.count})</span>
    </span>
  );
}

export default TeamThemeModal;
