/**
 * FairnessLensCard Component (V2.4.2)
 *
 * Displays demographic representation analysis with privacy guardrails.
 * Shows representation breakdown, rating parity, and promotion rates.
 *
 * Key guardrails:
 * - Group-level only (never individual analysis)
 * - Small-n suppression (groups < 5 hidden)
 * - Strong disclaimers on all outputs
 */

import { useState, useEffect, useCallback } from 'react';
import {
  isFairnessLensEnabled,
  getFairnessLensSummary,
} from '../../lib/tauri-commands';
import type {
  FairnessLensSummary,
  DeiBreakdown,
  RatingParityItem,
  PromotionRateItem,
} from '../../lib/dei-types';
import {
  formatPercentage,
  formatRating,
  hasSuppressedItems,
  hasRatingSuppression,
  hasPromotionSuppression,
} from '../../lib/dei-types';

interface FairnessLensCardProps {
  /** Optional: refresh trigger (increment to refetch) */
  refreshKey?: number;
}

type TabId = 'representation' | 'ratings' | 'promotions';
type DemographicView = 'gender' | 'ethnicity';

export function FairnessLensCard({ refreshKey = 0 }: FairnessLensCardProps) {
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [data, setData] = useState<FairnessLensSummary | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>('representation');
  const [demographicView, setDemographicView] = useState<DemographicView>('gender');

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      // Check if feature is enabled first
      const isEnabled = await isFairnessLensEnabled();
      setEnabled(isEnabled);

      if (!isEnabled) {
        setLoading(false);
        return;
      }

      // Fetch the DEI data
      const summary = await getFairnessLensSummary();
      setData(summary);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load fairness lens data';
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
          <span className="text-sm font-medium">Error loading fairness lens</span>
        </div>
        <p className="mt-1 text-xs text-red-600">{error}</p>
      </div>
    );
  }

  // Empty state
  if (!data) {
    return (
      <div className="p-4 bg-white border border-stone-200 rounded-xl">
        <h3 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-3">
          <svg className="w-4 h-4 text-teal-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
          Fairness Lens
        </h3>
        <p className="text-sm text-stone-500">
          No demographic data available. Ensure employees have gender or ethnicity data populated.
        </p>
      </div>
    );
  }

  return (
    <div className="p-4 bg-white border border-stone-200 rounded-xl">
      {/* Header */}
      <h3 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-3">
        <svg className="w-4 h-4 text-teal-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
        Fairness Lens
      </h3>

      {/* Disclaimer Banner - Always visible */}
      <div className="mb-4 p-3 bg-teal-50 border border-teal-200 rounded-lg">
        <div className="flex gap-2">
          <svg className="w-4 h-4 text-teal-600 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p className="text-xs text-teal-800">
            {data.disclaimer}
          </p>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex gap-1 mb-4 p-1 bg-stone-100 rounded-lg">
        <TabButton
          active={activeTab === 'representation'}
          onClick={() => setActiveTab('representation')}
        >
          Representation
        </TabButton>
        <TabButton
          active={activeTab === 'ratings'}
          onClick={() => setActiveTab('ratings')}
        >
          Rating Parity
        </TabButton>
        <TabButton
          active={activeTab === 'promotions'}
          onClick={() => setActiveTab('promotions')}
        >
          Promotions
        </TabButton>
      </div>

      {/* Demographic Toggle */}
      <div className="flex gap-2 mb-4">
        <button
          type="button"
          onClick={() => setDemographicView('gender')}
          className={`
            px-3 py-1 text-xs font-medium rounded-full transition-colors
            ${demographicView === 'gender'
              ? 'bg-teal-100 text-teal-700 border border-teal-200'
              : 'bg-stone-50 text-stone-600 border border-stone-200 hover:bg-stone-100'}
          `}
        >
          By Gender
        </button>
        <button
          type="button"
          onClick={() => setDemographicView('ethnicity')}
          className={`
            px-3 py-1 text-xs font-medium rounded-full transition-colors
            ${demographicView === 'ethnicity'
              ? 'bg-teal-100 text-teal-700 border border-teal-200'
              : 'bg-stone-50 text-stone-600 border border-stone-200 hover:bg-stone-100'}
          `}
        >
          By Ethnicity
        </button>
      </div>

      {/* Tab Content */}
      {activeTab === 'representation' && (
        <RepresentationView
          data={demographicView === 'gender'
            ? data.gender_representation
            : data.ethnicity_representation}
        />
      )}
      {activeTab === 'ratings' && (
        <RatingParityView
          data={demographicView === 'gender'
            ? data.gender_rating_parity
            : data.ethnicity_rating_parity}
        />
      )}
      {activeTab === 'promotions' && (
        <PromotionsView
          data={demographicView === 'gender'
            ? data.gender_promotion_rates
            : data.ethnicity_promotion_rates}
        />
      )}
    </div>
  );
}

interface TabButtonProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function TabButton({ active, onClick, children }: TabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`
        flex-1 px-3 py-1.5 text-xs font-medium rounded-md transition-colors
        ${active
          ? 'bg-white text-stone-800 shadow-sm'
          : 'text-stone-500 hover:text-stone-700'}
      `}
    >
      {children}
    </button>
  );
}

interface RepresentationViewProps {
  data: {
    breakdown: DeiBreakdown[];
    total: number;
  };
}

function RepresentationView({ data }: RepresentationViewProps) {
  const hasSuppressed = hasSuppressedItems(data.breakdown);

  return (
    <div className="space-y-2">
      {data.breakdown.map((item) => (
        <div
          key={item.label}
          className="flex items-center justify-between p-2 rounded-lg bg-stone-50"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm text-stone-700">{item.label}</span>
            {item.suppressed && (
              <SuppressedBadge />
            )}
          </div>
          <div className="text-right">
            {item.suppressed ? (
              <span className="text-sm text-stone-400">--</span>
            ) : (
              <>
                <span className="text-sm font-medium text-stone-800">
                  {formatPercentage(item.percentage)}
                </span>
                <span className="text-xs text-stone-500 ml-2">
                  ({item.count})
                </span>
              </>
            )}
          </div>
        </div>
      ))}

      {hasSuppressed && (
        <SuppressionNote />
      )}
    </div>
  );
}

interface RatingParityViewProps {
  data: {
    items: RatingParityItem[];
    overall_avg: number | null;
  };
}

function RatingParityView({ data }: RatingParityViewProps) {
  const hasSuppressed = hasRatingSuppression(data);

  return (
    <div className="space-y-2">
      {/* Overall average */}
      {data.overall_avg !== null && (
        <div className="mb-3 p-2 bg-stone-100 rounded-lg">
          <span className="text-xs text-stone-500">Overall average: </span>
          <span className="text-sm font-medium text-stone-800">
            {formatRating(data.overall_avg)}
          </span>
        </div>
      )}

      {/* Table header */}
      <div className="flex items-center justify-between px-2 text-xs text-stone-500 uppercase tracking-wide">
        <span>Group</span>
        <div className="flex gap-4">
          <span className="w-16 text-right">Avg Rating</span>
          <span className="w-12 text-right">Count</span>
        </div>
      </div>

      {/* Table rows */}
      {data.items.map((item) => (
        <div
          key={item.label}
          className="flex items-center justify-between p-2 rounded-lg bg-stone-50"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm text-stone-700">{item.label}</span>
            {item.suppressed && <SuppressedBadge />}
          </div>
          <div className="flex gap-4">
            <span className={`w-16 text-right text-sm ${item.suppressed ? 'text-stone-400' : 'font-medium text-stone-800'}`}>
              {formatRating(item.avg_rating)}
            </span>
            <span className={`w-12 text-right text-xs ${item.suppressed ? 'text-stone-400' : 'text-stone-500'}`}>
              {item.suppressed ? '--' : item.count}
            </span>
          </div>
        </div>
      ))}

      {hasSuppressed && (
        <SuppressionNote />
      )}
    </div>
  );
}

interface PromotionsViewProps {
  data: {
    items: PromotionRateItem[];
    overall_rate: number | null;
    disclaimer: string;
  };
}

function PromotionsView({ data }: PromotionsViewProps) {
  const hasSuppressed = hasPromotionSuppression(data);

  return (
    <div className="space-y-2">
      {/* Promotion disclaimer */}
      <div className="mb-3 p-2 bg-amber-50 border border-amber-100 rounded-lg">
        <p className="text-xs text-amber-700">{data.disclaimer}</p>
      </div>

      {/* Overall rate */}
      {data.overall_rate !== null && (
        <div className="mb-3 p-2 bg-stone-100 rounded-lg">
          <span className="text-xs text-stone-500">Overall promotion rate: </span>
          <span className="text-sm font-medium text-stone-800">
            {formatPercentage(data.overall_rate)}
          </span>
        </div>
      )}

      {/* Table header */}
      <div className="flex items-center justify-between px-2 text-xs text-stone-500 uppercase tracking-wide">
        <span>Group</span>
        <div className="flex gap-4">
          <span className="w-16 text-right">Rate</span>
          <span className="w-12 text-right">Count</span>
        </div>
      </div>

      {/* Table rows */}
      {data.items.map((item) => (
        <div
          key={item.label}
          className="flex items-center justify-between p-2 rounded-lg bg-stone-50"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm text-stone-700">{item.label}</span>
            {item.suppressed && <SuppressedBadge />}
          </div>
          <div className="flex gap-4">
            <span className={`w-16 text-right text-sm ${item.suppressed ? 'text-stone-400' : 'font-medium text-stone-800'}`}>
              {formatPercentage(item.rate)}
            </span>
            <span className={`w-12 text-right text-xs ${item.suppressed ? 'text-stone-400' : 'text-stone-500'}`}>
              {item.suppressed ? '--' : item.total_count}
            </span>
          </div>
        </div>
      ))}

      {hasSuppressed && (
        <SuppressionNote />
      )}
    </div>
  );
}

function SuppressedBadge() {
  return (
    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs text-stone-500 bg-stone-100 rounded border border-stone-200">
      <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
      </svg>
      &lt;5
    </span>
  );
}

function SuppressionNote() {
  return (
    <p className="mt-2 text-xs text-stone-500 flex items-center gap-1">
      <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
      </svg>
      Groups with fewer than 5 members are suppressed to protect privacy.
    </p>
  );
}

export default FairnessLensCard;
