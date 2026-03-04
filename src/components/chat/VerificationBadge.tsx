/**
 * VerificationBadge Component (V2.1.4)
 *
 * Displays verification status for aggregate query responses.
 * Shows a badge with the overall status and an expandable details panel
 * showing each numeric claim verified against ground truth.
 */

import { useState } from 'react';
import type { VerificationResult, ClaimType } from '../../lib/types';

interface VerificationBadgeProps {
  verification: VerificationResult;
}

const CLAIM_TYPE_LABELS: Record<ClaimType, string> = {
  TotalHeadcount: 'Headcount',
  ActiveCount: 'Active employees',
  DepartmentCount: 'Department size',
  AvgRating: 'Average rating',
  EnpsScore: 'eNPS score',
  TurnoverRate: 'Turnover rate',
  Percentage: 'Percentage',
};

export function VerificationBadge({ verification }: VerificationBadgeProps) {
  const [showDetails, setShowDetails] = useState(false);
  const [showSql, setShowSql] = useState(false);

  // Don't show badge for non-aggregate queries
  if (!verification.is_aggregate_query) {
    return null;
  }

  // Determine badge style based on status
  const isVerified = verification.overall_status === 'Verified';
  const isPartialMatch = verification.overall_status === 'PartialMatch';

  // Badge colors following existing patterns
  const badgeClasses = isVerified
    ? 'bg-primary-50 text-primary-700 hover:bg-primary-100'
    : isPartialMatch
    ? 'bg-amber-50 text-amber-700 hover:bg-amber-100'
    : 'bg-stone-100 text-stone-500 hover:bg-stone-200';

  const badgeLabel = isVerified
    ? 'Verified'
    : isPartialMatch
    ? 'Check Manually'
    : 'Unverified';

  // Icon based on status
  const icon = isVerified ? (
    <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20">
      <path
        fillRule="evenodd"
        d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
        clipRule="evenodd"
      />
    </svg>
  ) : (
    <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20">
      <path
        fillRule="evenodd"
        d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
        clipRule="evenodd"
      />
    </svg>
  );

  return (
    <div className="mt-2">
      {/* Badge button */}
      <button
        onClick={() => setShowDetails(!showDetails)}
        className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-full text-xs font-medium transition-colors ${badgeClasses}`}
        aria-expanded={showDetails}
        aria-label={`Verification status: ${badgeLabel}. Click to ${showDetails ? 'hide' : 'show'} details.`}
      >
        {icon}
        {badgeLabel}
        <svg
          className={`w-3 h-3 transition-transform ${showDetails ? 'rotate-180' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {/* Expandable details panel */}
      {showDetails && (
        <div className="mt-2 p-3 bg-stone-50 rounded-lg text-sm border border-stone-200">
          <div className="font-medium text-stone-900 mb-2">Verification Details</div>

          {verification.claims.length === 0 ? (
            <p className="text-stone-500 italic">
              No numeric claims detected in response.
            </p>
          ) : (
            <ul className="space-y-1.5">
              {verification.claims.map((claim, i) => (
                <li key={i} className="flex items-start gap-2">
                  <span
                    className={`flex-shrink-0 mt-0.5 ${
                      claim.is_match ? 'text-primary-600' : 'text-amber-600'
                    }`}
                  >
                    {claim.is_match ? '✓' : '⚠'}
                  </span>
                  <span className="text-stone-700">
                    <span className="font-medium">
                      {CLAIM_TYPE_LABELS[claim.claim_type]}:
                    </span>{' '}
                    {formatNumber(claim.value_found)}
                    {claim.ground_truth !== null && (
                      <span className="text-stone-500">
                        {' '}
                        (actual: {formatNumber(claim.ground_truth)})
                      </span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          )}

          {/* Show SQL toggle */}
          {verification.sql_query && (
            <div className="mt-3 pt-3 border-t border-stone-200">
              <button
                onClick={() => setShowSql(!showSql)}
                className="text-primary-600 hover:text-primary-700 text-xs font-medium"
              >
                {showSql ? 'Hide SQL' : 'Show SQL'}
              </button>
              {showSql && (
                <pre className="mt-2 p-2 bg-stone-800 text-stone-100 rounded text-xs overflow-x-auto whitespace-pre-wrap">
                  {verification.sql_query}
                </pre>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * Format a number for display
 */
function formatNumber(n: number): string {
  // Format percentages with % sign if they look like percentages
  if (Number.isInteger(n)) {
    return n.toString();
  }
  // Round to 2 decimal places
  return n.toFixed(2).replace(/\.?0+$/, '');
}

export default VerificationBadge;
