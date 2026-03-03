// People Partner - Employee Import Step (Step 4)
// Auto-loads sample data on first launch; offers option to import custom data

import { useState, useEffect, useCallback } from 'react';
import {
  bulkClearData,
  bulkImportReviewCycles,
  bulkImportEmployees,
  bulkImportRatings,
  bulkImportReviews,
  bulkImportEnps,
  listEmployees,
} from '../../../lib/tauri-commands';

// Import generated test data (bundled at build time via Vite)
import employeesData from '../../../../scripts/generated/employees.json';
import reviewCyclesData from '../../../../scripts/generated/review-cycles.json';
import ratingsData from '../../../../scripts/generated/ratings.json';
import reviewsData from '../../../../scripts/generated/reviews.json';
import enpsData from '../../../../scripts/generated/enps.json';

type ImportStatus = 'idle' | 'loading' | 'success' | 'error';

interface EmployeeImportStepProps {
  onContinue: () => void;
}

export function EmployeeImportStep({ onContinue }: EmployeeImportStepProps) {
  const [status, setStatus] = useState<ImportStatus>('idle');
  const [employeeCount, setEmployeeCount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);

  // Check for existing employees and auto-load sample data if none
  useEffect(() => {
    const checkAndLoad = async () => {
      try {
        // Check if we already have employees
        const result = await listEmployees({}, 1, 0);
        if (result.total > 0) {
          // Already have data
          setEmployeeCount(result.total);
          setStatus('success');
          return;
        }

        // No employees - auto-load sample data
        await loadSampleData();
      } catch (err) {
        console.error('[Onboarding] Employee check failed:', err);
        setError('Failed to check existing data');
        setStatus('error');
      }
    };

    checkAndLoad();
  }, []);

  const loadSampleData = useCallback(async () => {
    setStatus('loading');
    setProgress(0);
    setError(null);

    try {
      // Step 1: Clear any partial data
      setProgress(10);
      await bulkClearData();

      // Step 2: Import review cycles (must be first for FK references)
      setProgress(20);
      await bulkImportReviewCycles(reviewCyclesData);

      // Step 3: Import employees
      setProgress(40);
      await bulkImportEmployees(employeesData);

      // Step 4: Import performance ratings
      setProgress(60);
      await bulkImportRatings(ratingsData);

      // Step 5: Import performance reviews
      setProgress(75);
      await bulkImportReviews(reviewsData);

      // Step 6: Import eNPS responses
      setProgress(90);
      await bulkImportEnps(enpsData);

      // Done
      setProgress(100);
      setEmployeeCount(employeesData.length);
      setStatus('success');
    } catch (err) {
      console.error('[Onboarding] Sample data import failed:', err);
      setError(err instanceof Error ? err.message : 'Import failed');
      setStatus('error');
    }
  }, []);

  return (
    <div className="w-full">
      {/* Loading state */}
      {status === 'loading' && (
        <div className="text-center py-8">
          <div className="mb-4">
            <div className="w-16 h-16 mx-auto mb-4 relative">
              <svg className="w-16 h-16 text-primary-100" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
              <div className="absolute inset-0 flex items-center justify-center">
                <div className="w-8 h-8 border-4 border-primary-200 border-t-primary-500 rounded-full animate-spin-slow" />
              </div>
            </div>
          </div>
          <p className="text-stone-600 font-medium mb-2">Loading sample data...</p>
          <p className="text-sm text-stone-500 mb-4">This includes 100 employees with performance data</p>

          {/* Progress bar */}
          <div className="w-full max-w-xs mx-auto bg-stone-100 rounded-full h-2 overflow-hidden">
            <div
              className="h-full bg-primary-500 transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      )}

      {/* Success state */}
      {status === 'success' && (
        <div className="text-center py-6">
          <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-green-100 flex items-center justify-center">
            <svg className="w-8 h-8 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <p className="text-stone-800 font-medium mb-1">Sample data loaded!</p>
          <p className="text-sm text-stone-500 mb-6">
            {employeeCount} employees with performance ratings, reviews, and eNPS data
          </p>

          {/* Data summary */}
          <div className="bg-stone-50 rounded-xl p-4 mb-6 text-left">
            <h4 className="text-xs font-medium text-stone-500 uppercase tracking-wider mb-2">
              Acme Corp Sample Data
            </h4>
            <ul className="text-sm text-stone-600 space-y-1">
              <li className="flex items-center gap-2">
                <svg className="w-4 h-4 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                {employeesData.length} employees across 5 departments
              </li>
              <li className="flex items-center gap-2">
                <svg className="w-4 h-4 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
                {reviewCyclesData.length} review cycles with ratings
              </li>
              <li className="flex items-center gap-2">
                <svg className="w-4 h-4 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
                {enpsData.length} eNPS survey responses
              </li>
            </ul>
          </div>

          <button
            type="button"
            onClick={onContinue}
            className="w-full px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200"
          >
            Continue
          </button>

          <p className="mt-4 text-xs text-stone-500">
            You can import your own data later from the People tab
          </p>
        </div>
      )}

      {/* Error state */}
      {status === 'error' && (
        <div className="text-center py-6">
          <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-red-100 flex items-center justify-center">
            <svg className="w-8 h-8 text-red-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          <p className="text-stone-800 font-medium mb-1">Import failed</p>
          <p className="text-sm text-red-600 mb-6">{error}</p>

          <div className="flex gap-3 justify-center">
            <button
              type="button"
              onClick={loadSampleData}
              className="px-4 py-2 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-lg transition-colors"
            >
              Retry
            </button>
            <button
              type="button"
              onClick={onContinue}
              className="px-4 py-2 text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
            >
              Skip for now
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default EmployeeImportStep;
