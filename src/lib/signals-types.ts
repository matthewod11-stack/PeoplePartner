// HR Command Center - Signals Types (V2.4.1)
// Team-level attention signals for attrition and sentiment analysis

/**
 * Attention level based on composite score
 */
export type AttentionLevel = 'high' | 'moderate' | 'monitor' | 'low';

/**
 * Tenure factor breakdown
 */
export interface TenureFactor {
  /** Percentage of team with < 1 year tenure */
  pct_under_1yr: number;
  /** Percentage of team with 3-5 years tenure (career plateau window) */
  pct_3_to_5yr: number;
  /** Calculated factor score (0-100) */
  score: number;
}

/**
 * Performance factor breakdown
 */
export interface PerformanceFactor {
  /** Percentage of team with declining ratings (latest < previous) */
  pct_declining: number;
  /** Percentage of team with "needs improvement" rating (< 3.0) */
  pct_needs_improvement: number;
  /** Calculated factor score (0-100) */
  score: number;
}

/**
 * Engagement factor breakdown (based on eNPS)
 */
export interface EngagementFactor {
  /** Percentage of team that are detractors (eNPS <= 6) */
  pct_detractors: number;
  /** Percentage of team that are passives (eNPS 7-8) */
  pct_passives: number;
  /** Calculated factor score (0-100) */
  score: number;
}

/**
 * A theme occurrence from review highlights
 */
export interface ThemeOccurrence {
  /** Theme name (from VALID_THEMES) */
  theme: string;
  /** Dominant sentiment for this theme */
  sentiment: string;
  /** Number of occurrences in the team's reviews */
  count: number;
}

/**
 * Complete attention signal for a team
 */
export interface TeamAttentionSignal {
  /** Team/department name */
  team: string;
  /** Number of active employees */
  headcount: number;
  /** Composite attention score (0-100) */
  attention_score: number;
  /** Attention level category */
  attention_level: AttentionLevel;
  /** Tenure factor breakdown */
  tenure_factor: TenureFactor;
  /** Performance factor breakdown */
  performance_factor: PerformanceFactor;
  /** Engagement factor breakdown */
  engagement_factor: EngagementFactor;
  /** Common themes from recent reviews (top 3) */
  common_themes: ThemeOccurrence[];
}

/**
 * Summary of attention areas across all teams
 */
export interface AttentionAreasSummary {
  /** Teams with attention signals (filtered by MIN_TEAM_SIZE) */
  teams: TeamAttentionSignal[];
  /** Disclaimer text */
  disclaimer: string;
  /** When this was computed (ISO 8601) */
  computed_at: string;
}

/**
 * Get the top contributing factor for a team signal
 */
export function getTopFactor(
  signal: TeamAttentionSignal
): { name: string; value: string } {
  const factors = [
    { name: 'Tenure', score: signal.tenure_factor.score, value: '' },
    { name: 'Performance', score: signal.performance_factor.score, value: '' },
    { name: 'Engagement', score: signal.engagement_factor.score, value: '' },
  ];

  // Determine the highest scoring factor
  const top = factors.reduce((a, b) => (a.score > b.score ? a : b));

  // Format the value based on which factor it is
  switch (top.name) {
    case 'Tenure':
      return {
        name: 'Tenure',
        value: `${Math.round(signal.tenure_factor.pct_under_1yr)}% under 1 year`,
      };
    case 'Performance':
      if (signal.performance_factor.pct_declining > signal.performance_factor.pct_needs_improvement) {
        return {
          name: 'Performance',
          value: `${Math.round(signal.performance_factor.pct_declining)}% declining ratings`,
        };
      }
      return {
        name: 'Performance',
        value: `${Math.round(signal.performance_factor.pct_needs_improvement)}% needs improvement`,
      };
    case 'Engagement':
      return {
        name: 'Engagement',
        value: `${Math.round(signal.engagement_factor.pct_detractors)}% detractors`,
      };
    default:
      return { name: 'Unknown', value: '' };
  }
}

/**
 * Get the badge color class for an attention level
 */
export function getAttentionBadgeColor(level: AttentionLevel): string {
  switch (level) {
    case 'high':
      return 'bg-red-100 text-red-800 border-red-200';
    case 'moderate':
      return 'bg-amber-100 text-amber-800 border-amber-200';
    case 'monitor':
      return 'bg-stone-100 text-stone-800 border-stone-200';
    case 'low':
      return 'bg-green-100 text-green-800 border-green-200';
    default:
      return 'bg-stone-100 text-stone-800 border-stone-200';
  }
}

/**
 * Format attention score for display
 */
export function formatAttentionScore(score: number): string {
  return Math.round(score).toString();
}
