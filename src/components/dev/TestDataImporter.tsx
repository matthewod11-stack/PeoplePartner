// People Partner - Test Data Importer
// Development utility for loading generated test data

import { useState } from 'react';
import {
  bulkClearData,
  bulkImportReviewCycles,
  bulkImportEmployees,
  bulkImportRatings,
  bulkImportReviews,
  bulkImportEnps,
  verifyDataIntegrity,
  type BulkImportResult,
  type IntegrityCheckResult,
} from '../../lib/tauri-commands';

// Import generated test data (bundled at build time via Vite)
import employeesData from '../../../scripts/generated/employees.json';
import reviewCyclesData from '../../../scripts/generated/review-cycles.json';
import ratingsData from '../../../scripts/generated/ratings.json';
import reviewsData from '../../../scripts/generated/reviews.json';
import enpsData from '../../../scripts/generated/enps.json';

interface ImportStatus {
  step: string;
  status: 'pending' | 'running' | 'success' | 'error';
  result?: BulkImportResult;
  error?: string;
}

export function TestDataImporter() {
  const [isImporting, setIsImporting] = useState(false);
  const [importSteps, setImportSteps] = useState<ImportStatus[]>([]);
  const [integrityResults, setIntegrityResults] = useState<IntegrityCheckResult[]>([]);
  const [showConfirm, setShowConfirm] = useState(false);

  const updateStep = (step: string, update: Partial<ImportStatus>) => {
    setImportSteps(prev => prev.map(s =>
      s.step === step ? { ...s, ...update } : s
    ));
  };

  const handleImport = async () => {
    setShowConfirm(false);
    setIsImporting(true);
    setIntegrityResults([]);

    // Initialize steps
    const steps: ImportStatus[] = [
      { step: 'Clear existing data', status: 'pending' },
      { step: 'Import review cycles', status: 'pending' },
      { step: 'Import employees', status: 'pending' },
      { step: 'Import performance ratings', status: 'pending' },
      { step: 'Import performance reviews', status: 'pending' },
      { step: 'Import eNPS responses', status: 'pending' },
      { step: 'Verify integrity', status: 'pending' },
    ];
    setImportSteps(steps);

    try {
      // Step 1: Clear existing data
      updateStep('Clear existing data', { status: 'running' });
      await bulkClearData();
      updateStep('Clear existing data', { status: 'success' });

      // Step 2: Import review cycles (must be first for FK references)
      updateStep('Import review cycles', { status: 'running' });
      const cycleResult = await bulkImportReviewCycles(reviewCyclesData);
      updateStep('Import review cycles', {
        status: cycleResult.errors.length > 0 ? 'error' : 'success',
        result: cycleResult,
        error: cycleResult.errors.length > 0 ? cycleResult.errors.join(', ') : undefined
      });

      // Step 3: Import employees (must be before ratings/reviews/enps)
      updateStep('Import employees', { status: 'running' });
      const empResult = await bulkImportEmployees(employeesData);
      updateStep('Import employees', {
        status: empResult.errors.length > 0 ? 'error' : 'success',
        result: empResult,
        error: empResult.errors.length > 0 ? empResult.errors.join(', ') : undefined
      });

      // Step 4: Import performance ratings
      updateStep('Import performance ratings', { status: 'running' });
      const ratingResult = await bulkImportRatings(ratingsData);
      updateStep('Import performance ratings', {
        status: ratingResult.errors.length > 0 ? 'error' : 'success',
        result: ratingResult,
        error: ratingResult.errors.length > 0 ? ratingResult.errors.join(', ') : undefined
      });

      // Step 5: Import performance reviews
      updateStep('Import performance reviews', { status: 'running' });
      const reviewResult = await bulkImportReviews(reviewsData);
      updateStep('Import performance reviews', {
        status: reviewResult.errors.length > 0 ? 'error' : 'success',
        result: reviewResult,
        error: reviewResult.errors.length > 0 ? reviewResult.errors.join(', ') : undefined
      });

      // Step 6: Import eNPS responses
      updateStep('Import eNPS responses', { status: 'running' });
      const enpsResult = await bulkImportEnps(enpsData);
      updateStep('Import eNPS responses', {
        status: enpsResult.errors.length > 0 ? 'error' : 'success',
        result: enpsResult,
        error: enpsResult.errors.length > 0 ? enpsResult.errors.join(', ') : undefined
      });

      // Step 7: Verify integrity
      updateStep('Verify integrity', { status: 'running' });
      const integrity = await verifyDataIntegrity();
      setIntegrityResults(integrity);
      const allPassed = integrity.every(r => r.passed);
      updateStep('Verify integrity', {
        status: allPassed ? 'success' : 'error',
        error: allPassed ? undefined : 'Some integrity checks failed'
      });

    } catch (err) {
      console.error('Import failed:', err);
      setImportSteps(prev => prev.map(s =>
        s.status === 'running' ? { ...s, status: 'error', error: String(err) } : s
      ));
    } finally {
      setIsImporting(false);
    }
  };

  const getStatusIcon = (status: ImportStatus['status']) => {
    switch (status) {
      case 'pending': return '○';
      case 'running': return '◐';
      case 'success': return '●';
      case 'error': return '✗';
    }
  };

  const getStatusColor = (status: ImportStatus['status']) => {
    switch (status) {
      case 'pending': return 'text-gray-400';
      case 'running': return 'text-blue-500 animate-pulse';
      case 'success': return 'text-green-500';
      case 'error': return 'text-red-500';
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto">
      <h2 className="text-xl font-semibold mb-4">Test Data Importer</h2>

      <div className="bg-amber-50 border border-amber-200 rounded-lg p-4 mb-6">
        <p className="text-amber-800 text-sm">
          <strong>Development Tool:</strong> This will clear all existing data and load
          100 test employees with performance ratings, reviews, and eNPS responses.
        </p>
      </div>

      <div className="mb-6 space-y-2 text-sm text-gray-600">
        <p><strong>Data Summary:</strong></p>
        <ul className="list-disc ml-6">
          <li>{employeesData.length} employees</li>
          <li>{reviewCyclesData.length} review cycles</li>
          <li>{ratingsData.length} performance ratings</li>
          <li>{reviewsData.length} performance reviews</li>
          <li>{enpsData.length} eNPS responses</li>
        </ul>
      </div>

      {!showConfirm && importSteps.length === 0 && (
        <button
          onClick={() => setShowConfirm(true)}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          Load Test Data
        </button>
      )}

      {showConfirm && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 mb-4">
          <p className="text-red-800 mb-3">
            This will <strong>DELETE ALL EXISTING DATA</strong> and replace it with test data.
            Are you sure?
          </p>
          <div className="flex gap-2">
            <button
              onClick={handleImport}
              className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
            >
              Yes, Clear and Import
            </button>
            <button
              onClick={() => setShowConfirm(false)}
              className="px-4 py-2 bg-gray-200 text-gray-800 rounded hover:bg-gray-300"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {importSteps.length > 0 && (
        <div className="space-y-2 mb-6">
          {importSteps.map((step) => (
            <div key={step.step} className="flex items-center gap-3">
              <span className={`text-lg ${getStatusColor(step.status)}`}>
                {getStatusIcon(step.status)}
              </span>
              <span className={step.status === 'running' ? 'font-medium' : ''}>
                {step.step}
              </span>
              {step.result && (
                <span className="text-sm text-gray-500">
                  ({step.result.inserted} inserted)
                </span>
              )}
              {step.error && (
                <span className="text-sm text-red-600">{step.error}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {integrityResults.length > 0 && (
        <div className="mt-6">
          <h3 className="font-semibold mb-3">Integrity Check Results</h3>
          <div className="bg-gray-50 rounded-lg p-4 space-y-2 text-sm">
            {integrityResults.map((result, i) => (
              <div key={i} className="flex items-center gap-2">
                <span className={result.passed ? 'text-green-500' : 'text-red-500'}>
                  {result.passed ? '✓' : '✗'}
                </span>
                <span>{result.check_name}</span>
                <span className="text-gray-500">
                  (expected: {result.expected}, actual: {result.actual})
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {!isImporting && importSteps.length > 0 && (
        <button
          onClick={() => {
            setImportSteps([]);
            setIntegrityResults([]);
          }}
          className="mt-4 px-4 py-2 bg-gray-200 text-gray-800 rounded hover:bg-gray-300"
        >
          Reset
        </button>
      )}
    </div>
  );
}
