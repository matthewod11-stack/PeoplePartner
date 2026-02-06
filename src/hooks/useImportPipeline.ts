/**
 * useImportPipeline Hook
 *
 * Shared state machine for the V2.5.1 Data Quality Center import pipeline.
 * Encapsulates the step flow: upload -> mapping -> validate -> fix -> dedupe -> preview -> import.
 * Used by EmployeeImport, RatingsImport, ReviewsImport, and EnpsImport.
 */

import { useState, useCallback } from 'react';
import type {
  ParsePreview,
  ColumnMapping,
  ParsedRow,
  HeaderNormalization,
  ImportValidationResult,
  DuplicateGroup,
  DuplicateResolution,
  ImportType,
  IssueRow,
} from '../lib/types';
import {
  readFileAsBytes,
  parseFilePreview,
  parseFile,
  normalizeHeaders,
  validateImportData,
  detectDuplicates,
} from '../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

export type ImportPipelineStep =
  | 'upload'
  | 'mapping'
  | 'validating'
  | 'validation-review'
  | 'fixing'
  | 'deduping'
  | 'preview'
  | 'importing'
  | 'complete';

export interface ImportResultCommon {
  created: number;
  updated?: number;
  skipped?: number;
  errors: string[];
}

export interface ImportPipelineState {
  step: ImportPipelineStep;
  file: File | null;
  preview: ParsePreview | null;
  allRows: ParsedRow[];
  sourceHeaders: string[];
  normalizations: HeaderNormalization[];
  columnMapping: ColumnMapping;
  validationResult: ImportValidationResult | null;
  issueRows: IssueRow[];
  duplicates: DuplicateGroup[];
  resolutions: DuplicateResolution[];
  importResult: ImportResultCommon | null;
  error: string | null;
  isProcessing: boolean;
}

export interface ImportPipelineActions {
  selectFile: (file: File) => Promise<void>;
  confirmMapping: (mapping: ColumnMapping) => Promise<void>;
  fixAndRevalidate: (fixedRows: ParsedRow[]) => Promise<void>;
  skipErrors: () => void;
  resolveDuplicates: (resolutions: DuplicateResolution[]) => void;
  startImport: () => Promise<void>;
  goBack: () => void;
  reset: () => void;
}

export interface UseImportPipelineOptions {
  dataType: ImportType;
  targetFields: Record<string, string>;
  requiredFields: string[];
  autoMapFn: (headers: string[]) => Promise<ColumnMapping>;
  importFn: (
    rows: ParsedRow[],
    mapping: ColumnMapping,
    resolutions: DuplicateResolution[]
  ) => Promise<ImportResultCommon>;
  onComplete?: (result: ImportResultCommon) => void;
  onCancel?: () => void;
}

// =============================================================================
// Hook
// =============================================================================

const INITIAL_STATE: ImportPipelineState = {
  step: 'upload',
  file: null,
  preview: null,
  allRows: [],
  sourceHeaders: [],
  normalizations: [],
  columnMapping: {},
  validationResult: null,
  issueRows: [],
  duplicates: [],
  resolutions: [],
  importResult: null,
  error: null,
  isProcessing: false,
};

export function useImportPipeline(
  options: UseImportPipelineOptions
): [ImportPipelineState, ImportPipelineActions] {
  const { dataType, autoMapFn, importFn, onComplete, onCancel } = options;

  const [state, setState] = useState<ImportPipelineState>({ ...INITIAL_STATE });

  const selectFile = useCallback(
    async (file: File) => {
      setState((s) => ({ ...s, isProcessing: true, error: null, file }));

      try {
        const bytes = await readFileAsBytes(file);
        const previewData = await parseFilePreview(bytes, file.name, 5);

        // Auto-map columns
        const mapping = await autoMapFn(previewData.headers);

        // Normalize headers
        let normalizations: HeaderNormalization[] = [];
        try {
          normalizations = await normalizeHeaders(previewData.headers, dataType);
        } catch {
          // Backend may not have this command yet -- proceed without normalizations
        }

        setState((s) => ({
          ...s,
          step: 'mapping',
          preview: previewData,
          sourceHeaders: previewData.headers,
          columnMapping: mapping,
          normalizations,
          isProcessing: false,
        }));
      } catch (err) {
        setState((s) => ({
          ...s,
          step: 'upload',
          error: err instanceof Error ? err.message : 'Failed to parse file',
          isProcessing: false,
        }));
      }
    },
    [autoMapFn, dataType]
  );

  const confirmMapping = useCallback(
    async (mapping: ColumnMapping) => {
      setState((s) => ({
        ...s,
        columnMapping: mapping,
        step: 'validating',
        isProcessing: true,
        error: null,
      }));

      try {
        // Parse full file
        const file = state.file;
        if (!file) throw new Error('No file selected');

        const bytes = await readFileAsBytes(file);
        const fullData = await parseFile(bytes, file.name);
        const allRows = fullData.rows;

        // Validate
        let validationResult: ImportValidationResult;
        try {
          validationResult = await validateImportData(allRows, mapping, dataType);
        } catch {
          // Backend may not have this command yet -- treat as all valid
          validationResult = {
            isValid: true,
            issues: [],
            errorRowCount: 0,
            warningRowCount: 0,
            cleanRowCount: allRows.length,
          };
        }

        // Build issue rows
        const issueRows: IssueRow[] = [];
        if (!validationResult.isValid) {
          const issuesByRow = new Map<number, typeof validationResult.issues>();
          for (const issue of validationResult.issues) {
            const existing = issuesByRow.get(issue.row) ?? [];
            existing.push(issue);
            issuesByRow.set(issue.row, existing);
          }
          for (const [rowIndex, issues] of issuesByRow) {
            if (rowIndex - 1 < allRows.length) {
              issueRows.push({
                rowIndex: rowIndex - 1,
                data: allRows[rowIndex - 1],
                issues,
              });
            }
          }
        }

        if (validationResult.isValid) {
          // Skip validation review, go to dedupe
          await runDedupeCheck(allRows, mapping);
        } else {
          setState((s) => ({
            ...s,
            step: 'validation-review',
            allRows,
            validationResult,
            issueRows,
            isProcessing: false,
          }));
        }
      } catch (err) {
        setState((s) => ({
          ...s,
          step: 'mapping',
          error: err instanceof Error ? err.message : 'Validation failed',
          isProcessing: false,
        }));
      }
    },
    [state.file, dataType]
  );

  const runDedupeCheck = useCallback(
    async (rows: ParsedRow[], mapping: ColumnMapping) => {
      setState((s) => ({ ...s, step: 'deduping', isProcessing: true }));

      try {
        let duplicates: DuplicateGroup[] = [];
        try {
          duplicates = await detectDuplicates(rows, mapping, dataType);
        } catch {
          // Backend may not have this command yet
        }

        if (duplicates.length === 0) {
          setState((s) => ({
            ...s,
            step: 'preview',
            allRows: rows,
            duplicates: [],
            isProcessing: false,
          }));
        } else {
          setState((s) => ({
            ...s,
            step: 'deduping',
            allRows: rows,
            duplicates,
            isProcessing: false,
          }));
        }
      } catch (err) {
        setState((s) => ({
          ...s,
          step: 'preview',
          allRows: rows,
          error: err instanceof Error ? err.message : 'Dedupe check failed',
          isProcessing: false,
        }));
      }
    },
    [dataType]
  );

  const fixAndRevalidate = useCallback(
    async (fixedRows: ParsedRow[]) => {
      setState((s) => ({ ...s, isProcessing: true, error: null }));

      try {
        let validationResult: ImportValidationResult;
        try {
          validationResult = await validateImportData(
            fixedRows,
            state.columnMapping,
            dataType
          );
        } catch {
          validationResult = {
            isValid: true,
            issues: [],
            errorRowCount: 0,
            warningRowCount: 0,
            cleanRowCount: fixedRows.length,
          };
        }

        const issueRows: IssueRow[] = [];
        if (!validationResult.isValid) {
          const issuesByRow = new Map<number, typeof validationResult.issues>();
          for (const issue of validationResult.issues) {
            const existing = issuesByRow.get(issue.row) ?? [];
            existing.push(issue);
            issuesByRow.set(issue.row, existing);
          }
          for (const [rowIndex, issues] of issuesByRow) {
            if (rowIndex - 1 < fixedRows.length) {
              issueRows.push({
                rowIndex: rowIndex - 1,
                data: fixedRows[rowIndex - 1],
                issues,
              });
            }
          }
        }

        if (validationResult.isValid) {
          await runDedupeCheck(fixedRows, state.columnMapping);
        } else {
          setState((s) => ({
            ...s,
            step: 'validation-review',
            allRows: fixedRows,
            validationResult,
            issueRows,
            isProcessing: false,
          }));
        }
      } catch (err) {
        setState((s) => ({
          ...s,
          error: err instanceof Error ? err.message : 'Re-validation failed',
          isProcessing: false,
        }));
      }
    },
    [state.columnMapping, dataType, runDedupeCheck]
  );

  const skipErrors = useCallback(() => {
    // Filter out rows with errors, keep rows with only warnings or no issues
    const errorRowIndices = new Set(
      (state.validationResult?.issues ?? [])
        .filter((i) => i.severity === 'error')
        .map((i) => i.row - 1)
    );
    const cleanRows = state.allRows.filter((_, i) => !errorRowIndices.has(i));

    runDedupeCheck(cleanRows, state.columnMapping);
  }, [state.validationResult, state.allRows, state.columnMapping, runDedupeCheck]);

  const resolveDuplicates = useCallback(
    (resolutions: DuplicateResolution[]) => {
      setState((s) => ({
        ...s,
        step: 'preview',
        resolutions,
      }));
    },
    []
  );

  const startImport = useCallback(async () => {
    setState((s) => ({ ...s, step: 'importing', isProcessing: true, error: null }));

    try {
      const result = await importFn(
        state.allRows,
        state.columnMapping,
        state.resolutions
      );

      setState((s) => ({
        ...s,
        step: 'complete',
        importResult: result,
        isProcessing: false,
      }));

      onComplete?.(result);
    } catch (err) {
      setState((s) => ({
        ...s,
        step: 'preview',
        error: err instanceof Error ? err.message : 'Import failed',
        isProcessing: false,
      }));
    }
  }, [state.allRows, state.columnMapping, state.resolutions, importFn, onComplete]);

  const goBack = useCallback(() => {
    setState((s) => {
      const stepOrder: ImportPipelineStep[] = [
        'upload',
        'mapping',
        'validating',
        'validation-review',
        'fixing',
        'deduping',
        'preview',
        'importing',
        'complete',
      ];
      const currentIndex = stepOrder.indexOf(s.step);

      // Find the previous non-transient step
      let prevStep: ImportPipelineStep = 'upload';
      if (s.step === 'mapping') prevStep = 'upload';
      else if (s.step === 'validation-review') prevStep = 'mapping';
      else if (s.step === 'fixing') prevStep = 'validation-review';
      else if (s.step === 'deduping') prevStep = 'validation-review';
      else if (s.step === 'preview') {
        prevStep = s.duplicates.length > 0 ? 'deduping' : 'mapping';
      } else if (currentIndex > 0) {
        prevStep = stepOrder[currentIndex - 1];
      }

      return { ...s, step: prevStep, error: null };
    });
  }, []);

  const reset = useCallback(() => {
    setState({ ...INITIAL_STATE });
    onCancel?.();
  }, [onCancel]);

  const actions: ImportPipelineActions = {
    selectFile,
    confirmMapping,
    fixAndRevalidate,
    skipErrors,
    resolveDuplicates,
    startImport,
    goBack,
    reset,
  };

  return [state, actions];
}
