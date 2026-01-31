// HR Command Center - DEI & Fairness Lens Types (V2.4.2)
// Demographic representation analysis with privacy guardrails

/**
 * A single demographic breakdown item
 */
export interface DeiBreakdown {
  /** Category label (e.g., "Female", "Male", "Engineering") */
  label: string;
  /** Count of employees in this category */
  count: number;
  /** Percentage of total */
  percentage: number;
  /** Whether this group is suppressed due to small size (<5) */
  suppressed: boolean;
}

/**
 * Representation breakdown result
 */
export interface RepresentationResult {
  /** The grouping dimension (e.g., "gender", "ethnicity") */
  group_by: string;
  /** Filter applied (e.g., department name or null for all) */
  filter_department: string | null;
  /** Breakdown items */
  breakdown: DeiBreakdown[];
  /** Total count (including suppressed groups) */
  total: number;
  /** Disclaimer text */
  disclaimer: string;
  /** When this was computed (ISO 8601) */
  computed_at: string;
}

/**
 * Rating parity item showing average rating by demographic
 */
export interface RatingParityItem {
  /** Category label */
  label: string;
  /** Number of employees with ratings */
  count: number;
  /** Average rating for this group (null if suppressed) */
  avg_rating: number | null;
  /** Whether this group is suppressed due to small size */
  suppressed: boolean;
}

/**
 * Rating parity result
 */
export interface RatingParityResult {
  /** The grouping dimension (e.g., "gender", "ethnicity") */
  group_by: string;
  /** Parity items by group */
  items: RatingParityItem[];
  /** Overall average rating */
  overall_avg: number | null;
  /** Disclaimer text */
  disclaimer: string;
  /** When this was computed (ISO 8601) */
  computed_at: string;
}

/**
 * Promotion rate item by demographic
 */
export interface PromotionRateItem {
  /** Category label */
  label: string;
  /** Number of employees in group */
  total_count: number;
  /** Number with promotion-indicating titles */
  promoted_count: number;
  /** Promotion rate percentage (null if suppressed) */
  rate: number | null;
  /** Whether this group is suppressed due to small size */
  suppressed: boolean;
}

/**
 * Promotion rates result
 */
export interface PromotionRatesResult {
  /** The grouping dimension (e.g., "gender", "ethnicity") */
  group_by: string;
  /** Rates by group */
  items: PromotionRateItem[];
  /** Overall promotion rate */
  overall_rate: number | null;
  /** Promotion data disclaimer */
  disclaimer: string;
  /** When this was computed (ISO 8601) */
  computed_at: string;
}

/**
 * Complete fairness lens summary
 */
export interface FairnessLensSummary {
  /** Representation by gender */
  gender_representation: RepresentationResult;
  /** Representation by ethnicity */
  ethnicity_representation: RepresentationResult;
  /** Rating parity by gender */
  gender_rating_parity: RatingParityResult;
  /** Rating parity by ethnicity */
  ethnicity_rating_parity: RatingParityResult;
  /** Promotion rates by gender */
  gender_promotion_rates: PromotionRatesResult;
  /** Promotion rates by ethnicity */
  ethnicity_promotion_rates: PromotionRatesResult;
  /** Main disclaimer */
  disclaimer: string;
  /** When this was computed (ISO 8601) */
  computed_at: string;
}

/**
 * Group by options for DEI queries
 */
export type DeiGroupBy = 'gender' | 'ethnicity';

/**
 * Get badge color class for suppressed items
 */
export function getSuppressedBadgeColor(): string {
  return 'bg-stone-100 text-stone-500 border-stone-200';
}

/**
 * Format a percentage for display
 */
export function formatPercentage(value: number | null): string {
  if (value === null) return '--';
  return `${value.toFixed(1)}%`;
}

/**
 * Format a rating for display
 */
export function formatRating(value: number | null): string {
  if (value === null) return '--';
  return value.toFixed(2);
}

/**
 * Check if any items in a breakdown are suppressed
 */
export function hasSuppressedItems(breakdown: DeiBreakdown[]): boolean {
  return breakdown.some(item => item.suppressed);
}

/**
 * Check if any items in a rating parity result are suppressed
 */
export function hasRatingSuppression(data: { items: RatingParityItem[] }): boolean {
  return data.items.some(item => item.suppressed);
}

/**
 * Check if any items in a promotion rates result are suppressed
 */
export function hasPromotionSuppression(data: { items: PromotionRateItem[] }): boolean {
  return data.items.some(item => item.suppressed);
}
