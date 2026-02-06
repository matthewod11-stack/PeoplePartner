import { useState, useCallback } from 'react';
import type {
  ImportType,
  HeaderNormalization,
  HrisPreset,
  HrisPresetId,
  ImportValidationResult,
  DuplicateGroup,
  DuplicateResolution,
  ColumnMapping,
  ParsedRow,
} from '../lib/types';
import {
  normalizeHeaders,
  getHrisPresets,
  validateImportData,
  detectDuplicates,
} from '../lib/tauri-commands';

export type DataQualityStep = 'mapping' | 'validating' | 'deduping' | 'ready';

interface DataQualityState {
  step: DataQualityStep;
  headerNormalizations: HeaderNormalization[];
  hrisPresets: HrisPreset[];
  selectedPreset: HrisPresetId | null;
  /** User's column mapping: target_field -> source_header */
  columnMapping: ColumnMapping;
  validationResult: ImportValidationResult | null;
  duplicateGroups: DuplicateGroup[];
  duplicateResolutions: DuplicateResolution[];
  isLoading: boolean;
  error: string | null;
}

const initialState: DataQualityState = {
  step: 'mapping',
  headerNormalizations: [],
  hrisPresets: [],
  selectedPreset: null,
  columnMapping: {},
  validationResult: null,
  duplicateGroups: [],
  duplicateResolutions: [],
  isLoading: false,
  error: null,
};

export function useDataQuality(dataType: ImportType) {
  const [state, setState] = useState<DataQualityState>(initialState);

  /** Initialize: analyze headers and load HRIS presets */
  const analyzeHeaders = useCallback(async (headers: string[]) => {
    setState(s => ({ ...s, isLoading: true, error: null }));
    try {
      const [normalizations, presets] = await Promise.all([
        normalizeHeaders(headers, dataType),
        getHrisPresets(),
      ]);

      // Build initial mapping from auto-detected fields
      const autoMapping: ColumnMapping = {};
      for (const norm of normalizations) {
        if (norm.detectedField && norm.confidence >= 0.5) {
          autoMapping[norm.detectedField] = norm.original;
        }
      }

      setState(s => ({
        ...s,
        step: 'mapping',
        headerNormalizations: normalizations,
        hrisPresets: presets,
        columnMapping: autoMapping,
        isLoading: false,
      }));
    } catch (err) {
      setState(s => ({
        ...s,
        isLoading: false,
        error: err instanceof Error ? err.message : 'Failed to analyze headers',
      }));
    }
  }, [dataType]);

  /** Apply an HRIS preset to re-normalize headers */
  const applyPreset = useCallback(async (presetId: HrisPresetId | null, headers: string[]) => {
    setState(s => ({ ...s, isLoading: true, selectedPreset: presetId }));
    try {
      const normalizations = await normalizeHeaders(headers, dataType, presetId);

      const autoMapping: ColumnMapping = {};
      for (const norm of normalizations) {
        if (norm.detectedField && norm.confidence >= 0.5) {
          autoMapping[norm.detectedField] = norm.original;
        }
      }

      setState(s => ({
        ...s,
        headerNormalizations: normalizations,
        columnMapping: autoMapping,
        isLoading: false,
      }));
    } catch (err) {
      setState(s => ({
        ...s,
        isLoading: false,
        error: err instanceof Error ? err.message : 'Failed to apply preset',
      }));
    }
  }, [dataType]);

  /** Update a single column mapping */
  const updateMapping = useCallback((targetField: string, sourceHeader: string | null) => {
    setState(s => {
      const newMapping = { ...s.columnMapping };
      if (sourceHeader) {
        newMapping[targetField] = sourceHeader;
      } else {
        delete newMapping[targetField];
      }
      return { ...s, columnMapping: newMapping };
    });
  }, []);

  /** Run validation on the data */
  const validate = useCallback(async (rows: ParsedRow[]) => {
    setState(s => ({ ...s, step: 'validating', isLoading: true, error: null }));
    try {
      const result = await validateImportData(
        rows as Record<string, string>[],
        state.columnMapping,
        dataType,
      );

      setState(s => ({
        ...s,
        validationResult: result,
        isLoading: false,
        // Auto-advance if valid
        step: result.isValid ? 'deduping' : 'validating',
      }));

      return result;
    } catch (err) {
      setState(s => ({
        ...s,
        isLoading: false,
        error: err instanceof Error ? err.message : 'Validation failed',
      }));
      return null;
    }
  }, [dataType, state.columnMapping]);

  /** Run duplicate detection */
  const checkDuplicates = useCallback(async (rows: ParsedRow[]) => {
    setState(s => ({ ...s, step: 'deduping', isLoading: true, error: null }));
    try {
      const groups = await detectDuplicates(
        rows as Record<string, string>[],
        state.columnMapping,
        dataType,
      );

      setState(s => ({
        ...s,
        duplicateGroups: groups,
        isLoading: false,
        // Auto-advance if no duplicates
        step: groups.length === 0 ? 'ready' : 'deduping',
      }));

      return groups;
    } catch (err) {
      setState(s => ({
        ...s,
        isLoading: false,
        error: err instanceof Error ? err.message : 'Duplicate detection failed',
      }));
      return [];
    }
  }, [dataType, state.columnMapping]);

  /** Resolve a duplicate */
  const resolveDuplicate = useCallback((resolution: DuplicateResolution) => {
    setState(s => ({
      ...s,
      duplicateResolutions: [
        ...s.duplicateResolutions.filter(r => r.groupId !== resolution.groupId),
        resolution,
      ],
    }));
  }, []);

  /** Mark deduplication as complete */
  const finishDeduplication = useCallback(() => {
    setState(s => ({ ...s, step: 'ready' }));
  }, []);

  /** Go back to a previous step */
  const goToStep = useCallback((step: DataQualityStep) => {
    setState(s => ({ ...s, step }));
  }, []);

  /** Reset state */
  const reset = useCallback(() => {
    setState(initialState);
  }, []);

  return {
    ...state,
    analyzeHeaders,
    applyPreset,
    updateMapping,
    validate,
    checkDuplicates,
    resolveDuplicate,
    finishDeduplication,
    goToStep,
    reset,
  };
}
