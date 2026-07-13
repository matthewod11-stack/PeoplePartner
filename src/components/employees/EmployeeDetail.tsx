/**
 * Employee Detail Component
 *
 * Main employee profile view showing header, info sections, and performance data.
 * Composed from smaller subcomponents in ./detail/
 */

import { useEffect, useState } from 'react';
import { useEmployees } from '../../contexts/EmployeeContext';
import type {
  PerformanceRating,
  PerformanceReview,
  EnpsResponse,
} from '../../lib/types';
import {
  getRatingsForEmployee,
  getReviewsForEmployee,
  getEnpsForEmployee,
} from '../../lib/tauri-commands';

// Subcomponents
import { PrepBriefModal } from '../people_map';
import {
  EmployeeHeader,
  DetailsSection,
  DemographicsSection,
  TerminationSection,
  RatingsSection,
  EnpsSection,
  ReviewsSection,
  PerformanceLoadingSkeleton,
  NoPerformanceData,
  RatingDetailModal,
  EnpsDetailModal,
  ReviewDetailModal,
} from './detail';

// =============================================================================
// Empty State
// =============================================================================

function EmptyState() {
  return (
    <div className="h-full flex flex-col items-center justify-center p-4 text-center">
      <svg
        className="w-16 h-16 text-stone-200 mb-4"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={1}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M17.982 18.725A7.488 7.488 0 0012 15.75a7.488 7.488 0 00-5.982 2.975m11.963 0a9 9 0 10-11.963 0m11.963 0A8.966 8.966 0 0112 21a8.966 8.966 0 01-5.982-2.275M15 9.75a3 3 0 11-6 0 3 3 0 016 0z"
        />
      </svg>
      <p className="text-stone-500 text-sm">Select an employee</p>
      <p className="text-stone-500 text-xs mt-1">to view their details</p>
    </div>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function EmployeeDetail() {
  const { selectedEmployee, selectedEmployeeId, openEditModal, employees } =
    useEmployees();

  // Performance data state
  const [ratings, setRatings] = useState<PerformanceRating[]>([]);
  const [reviews, setReviews] = useState<PerformanceReview[]>([]);
  const [enpsResponses, setEnpsResponses] = useState<EnpsResponse[]>([]);
  const [isLoadingPerformance, setIsLoadingPerformance] = useState(false);

  // Modal state
  const [selectedRating, setSelectedRating] =
    useState<PerformanceRating | null>(null);
  const [selectedEnps, setSelectedEnps] = useState<EnpsResponse | null>(null);
  const [selectedReviewIndex, setSelectedReviewIndex] = useState<number | null>(
    null
  );
  const [isPrepBriefOpen, setIsPrepBriefOpen] = useState(false);

  // Fetch performance data when employee changes
  useEffect(() => {
    if (!selectedEmployeeId) {
      setRatings([]);
      setReviews([]);
      setEnpsResponses([]);
      return;
    }

    const fetchPerformanceData = async () => {
      setIsLoadingPerformance(true);
      try {
        const [ratingsData, reviewsData, enpsData] = await Promise.all([
          getRatingsForEmployee(selectedEmployeeId),
          getReviewsForEmployee(selectedEmployeeId),
          getEnpsForEmployee(selectedEmployeeId),
        ]);
        setRatings(ratingsData);
        setReviews(reviewsData);
        setEnpsResponses(enpsData);
      } catch (error) {
        console.error('Failed to load performance data:', error);
      } finally {
        setIsLoadingPerformance(false);
      }
    };

    fetchPerformanceData();
  }, [selectedEmployeeId]);

  // No employee selected
  if (!selectedEmployee) {
    return <EmptyState />;
  }

  // Derived data
  const latestRating = ratings[0];
  const latestEnps = enpsResponses[0];
  const manager = selectedEmployee.manager_id
    ? employees.find((e) => e.id === selectedEmployee.manager_id)
    : null;

  const hasPerformanceData =
    ratings.length > 0 || enpsResponses.length > 0 || reviews.length > 0;

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Header */}
      <EmployeeHeader
        employee={selectedEmployee}
        latestRating={latestRating}
        latestEnps={latestEnps}
        onEdit={openEditModal}
        onPrepBrief={() => setIsPrepBriefOpen(true)}
      />

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* Info sections */}
        <DetailsSection
          employee={selectedEmployee}
          managerName={manager?.full_name}
        />
        <DemographicsSection employee={selectedEmployee} />
        <TerminationSection employee={selectedEmployee} />

        {/* Performance data */}
        {isLoadingPerformance ? (
          <PerformanceLoadingSkeleton />
        ) : (
          <>
            <RatingsSection
              ratings={ratings}
              onSelectRating={setSelectedRating}
            />
            <EnpsSection
              responses={enpsResponses}
              onSelectResponse={setSelectedEnps}
            />
            <ReviewsSection
              reviews={reviews}
              onSelectReview={setSelectedReviewIndex}
            />
            {!hasPerformanceData && <NoPerformanceData />}
          </>
        )}
      </div>

      {/* Modals */}
      <RatingDetailModal
        rating={selectedRating}
        onClose={() => setSelectedRating(null)}
      />
      <EnpsDetailModal
        response={selectedEnps}
        onClose={() => setSelectedEnps(null)}
      />
      <ReviewDetailModal
        reviews={reviews}
        selectedIndex={selectedReviewIndex}
        onClose={() => setSelectedReviewIndex(null)}
        onNavigate={setSelectedReviewIndex}
      />
      <PrepBriefModal
        employee={isPrepBriefOpen ? selectedEmployee : null}
        onClose={() => setIsPrepBriefOpen(false)}
      />
    </div>
  );
}

export default EmployeeDetail;
