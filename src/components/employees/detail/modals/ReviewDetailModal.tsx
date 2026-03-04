/**
 * Review Detail Modal
 *
 * Modal showing full performance review with navigation between reviews.
 */

import type { PerformanceReview } from '../../../../lib/types';
import { Modal } from '../../../shared';
import { formatDate } from '../../../ui';

interface ReviewDetailModalProps {
  reviews: PerformanceReview[];
  selectedIndex: number | null;
  onClose: () => void;
  onNavigate: (index: number) => void;
}

export function ReviewDetailModal({
  reviews,
  selectedIndex,
  onClose,
  onNavigate,
}: ReviewDetailModalProps) {
  const selectedReview =
    selectedIndex !== null ? reviews[selectedIndex] : null;

  return (
    <Modal
      isOpen={selectedIndex !== null}
      onClose={onClose}
      title="Performance Review"
      maxWidth="max-w-2xl"
    >
      {selectedReview && selectedIndex !== null && (
        <div className="space-y-5">
          {/* Navigation header */}
          <div className="flex items-center justify-between">
            <button
              onClick={() => onNavigate(Math.max(0, selectedIndex - 1))}
              disabled={selectedIndex === 0}
              className="p-2.5 rounded-lg text-stone-500 hover:text-stone-700 hover:bg-stone-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              aria-label="Previous review"
            >
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M15.75 19.5L8.25 12l7.5-7.5"
                />
              </svg>
            </button>
            <span className="text-sm text-stone-500">
              {selectedIndex + 1} of {reviews.length}
            </span>
            <button
              onClick={() =>
                onNavigate(Math.min(reviews.length - 1, selectedIndex + 1))
              }
              disabled={selectedIndex === reviews.length - 1}
              className="p-2.5 rounded-lg text-stone-500 hover:text-stone-700 hover:bg-stone-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              aria-label="Next review"
            >
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M8.25 4.5l7.5 7.5-7.5 7.5"
                />
              </svg>
            </button>
          </div>

          <div className="flex items-center justify-between text-sm">
            <span className="text-stone-500">Review Date</span>
            <span className="font-medium">
              {formatDate(selectedReview.review_date)}
            </span>
          </div>

          {selectedReview.strengths && (
            <div>
              <h4 className="text-sm font-medium text-stone-700 mb-2">
                Strengths
              </h4>
              <p className="text-stone-600 text-sm whitespace-pre-wrap bg-primary-50 rounded-lg p-3 border border-primary-100">
                {selectedReview.strengths}
              </p>
            </div>
          )}

          {selectedReview.areas_for_improvement && (
            <div>
              <h4 className="text-sm font-medium text-stone-700 mb-2">
                Areas for Improvement
              </h4>
              <p className="text-stone-600 text-sm whitespace-pre-wrap bg-amber-50 rounded-lg p-3 border border-amber-100">
                {selectedReview.areas_for_improvement}
              </p>
            </div>
          )}

          {selectedReview.goals_next_period && (
            <div>
              <h4 className="text-sm font-medium text-stone-700 mb-2">
                Goals for Next Period
              </h4>
              <p className="text-stone-600 text-sm whitespace-pre-wrap bg-blue-50 rounded-lg p-3 border border-blue-100">
                {selectedReview.goals_next_period}
              </p>
            </div>
          )}

          {selectedReview.manager_comments && (
            <div>
              <h4 className="text-sm font-medium text-stone-700 mb-2">
                Manager Comments
              </h4>
              <p className="text-stone-600 text-sm whitespace-pre-wrap bg-stone-50 rounded-lg p-3 border border-stone-200">
                {selectedReview.manager_comments}
              </p>
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
