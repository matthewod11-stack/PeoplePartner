/**
 * ColumnMappingStep Component (V2.5.1a + V2.5.1b + V2.5.1f)
 *
 * After file parse, shows source headers mapped to target fields via dropdowns.
 * Includes HRIS preset selector and header normalization preview.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import type {
  ColumnMapping,
  HeaderNormalization,
  HrisPreset,
  HrisPresetId,
  ImportType,
} from '../../lib/types';
import { getHrisPresets, normalizeHeaders } from '../../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

interface ColumnMappingStepProps {
  sourceHeaders: string[];
  targetFields: Record<string, string>;
  requiredFields: string[];
  initialMapping: ColumnMapping;
  normalizations: HeaderNormalization[];
  dataType: ImportType;
  onConfirm: (mapping: ColumnMapping) => void;
  onBack: () => void;
}

// =============================================================================
// Main Component
// =============================================================================

export function ColumnMappingStep({
  sourceHeaders,
  targetFields,
  requiredFields,
  initialMapping,
  normalizations,
  dataType,
  onConfirm,
  onBack,
}: ColumnMappingStepProps) {
  const [mapping, setMapping] = useState<ColumnMapping>(initialMapping);
  const [presets, setPresets] = useState<HrisPreset[]>([]);
  const [selectedPreset, setSelectedPreset] = useState<HrisPresetId | null>(null);
  const [showNormalization, setShowNormalization] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);

  // Focus heading on mount for accessibility
  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  // Load HRIS presets
  useEffect(() => {
    getHrisPresets()
      .then(setPresets)
      .catch(() => {
        // Backend may not have this command yet
      });
  }, []);

  // Check if all required fields are mapped
  const unmappedRequired = requiredFields.filter((f) => !mapping[f]);
  const canContinue = unmappedRequired.length === 0;

  // Get which source headers are already used in a mapping
  const usedHeaders = new Set(Object.values(mapping).filter(Boolean));

  const handleFieldChange = useCallback(
    (fieldKey: string, sourceHeader: string) => {
      setMapping((prev) => {
        const next = { ...prev };
        if (sourceHeader === '') {
          delete next[fieldKey];
        } else {
          next[fieldKey] = sourceHeader;
        }
        return next;
      });
    },
    []
  );

  const handlePresetSelect = useCallback(
    async (presetId: HrisPresetId | '') => {
      if (!presetId) {
        setSelectedPreset(null);
        return;
      }

      setSelectedPreset(presetId);

      // Try to get normalized mapping from backend
      try {
        const norms = await normalizeHeaders(sourceHeaders, dataType, presetId);
        const newMapping: ColumnMapping = {};
        for (const norm of norms) {
          if (norm.detectedField && norm.confidence > 0.5) {
            newMapping[norm.detectedField] = norm.original;
          }
        }
        // Merge: preset fills in unmapped fields, doesn't override existing
        setMapping((prev) => {
          const merged = { ...prev };
          for (const [field, header] of Object.entries(newMapping)) {
            if (!merged[field]) {
              merged[field] = header;
            }
          }
          return merged;
        });
      } catch {
        // Fall back to preset's static mappings
        const preset = presets.find((p) => p.id === presetId);
        if (preset) {
          const newMapping: ColumnMapping = {};
          for (const [field, candidates] of Object.entries(preset.mappings)) {
            const match = candidates.find((c) =>
              sourceHeaders.some(
                (h) => h.toLowerCase().trim() === c.toLowerCase().trim()
              )
            );
            if (match) {
              const actualHeader = sourceHeaders.find(
                (h) => h.toLowerCase().trim() === match.toLowerCase().trim()
              );
              if (actualHeader) {
                newMapping[field] = actualHeader;
              }
            }
          }
          setMapping((prev) => {
            const merged = { ...prev };
            for (const [field, header] of Object.entries(newMapping)) {
              if (!merged[field]) {
                merged[field] = header;
              }
            }
            return merged;
          });
        }
      }
    },
    [sourceHeaders, dataType, presets]
  );

  const handleAutoDetect = useCallback(() => {
    // Reset mapping to auto-detected values from normalizations
    const autoMapping: ColumnMapping = {};
    for (const norm of normalizations) {
      if (norm.detectedField && norm.confidence > 0.5) {
        autoMapping[norm.detectedField] = norm.original;
      }
    }
    setMapping(autoMapping);
  }, [normalizations]);

  const handleClearAll = useCallback(() => {
    setMapping({});
    setSelectedPreset(null);
  }, []);

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      {/* Header */}
      <div className="px-6 py-4 border-b border-stone-200 bg-stone-50">
        <h3
          ref={headingRef}
          tabIndex={-1}
          className="text-lg font-medium text-stone-900 outline-none"
        >
          Map Columns
        </h3>
        <p className="mt-1 text-sm text-stone-500">
          Match your file's columns to the expected fields
        </p>
      </div>

      <div className="p-6 space-y-6">
        {/* HRIS Preset Selector */}
        {presets.length > 0 && (
          <HrisPresetSelector
            presets={presets}
            selectedPreset={selectedPreset}
            onSelect={handlePresetSelect}
          />
        )}

        {/* Mapping Table */}
        <div role="table" aria-label="Column Mapping">
          <div
            role="row"
            className="grid grid-cols-[1fr_auto_1fr] gap-3 items-center px-3 py-2 text-xs font-semibold text-stone-500 uppercase tracking-wider"
          >
            <div role="columnheader">Target Field</div>
            <div role="columnheader" className="w-8" />
            <div role="columnheader">Source Column</div>
          </div>

          <div className="space-y-1">
            {Object.entries(targetFields).map(([fieldKey, fieldLabel]) => {
              const isRequired = requiredFields.includes(fieldKey);
              const currentValue = mapping[fieldKey] || '';
              const normHint = normalizations.find(
                (n) => n.detectedField === fieldKey
              );

              return (
                <MappingRow
                  key={fieldKey}
                  fieldKey={fieldKey}
                  fieldLabel={fieldLabel}
                  isRequired={isRequired}
                  isMapped={!!currentValue}
                  sourceHeaders={sourceHeaders}
                  usedHeaders={usedHeaders}
                  currentValue={currentValue}
                  normHint={normHint}
                  onChange={(header) => handleFieldChange(fieldKey, header)}
                />
              );
            })}
          </div>
        </div>

        {/* Unmapped Required Warning */}
        {unmappedRequired.length > 0 && (
          <div className="flex items-start gap-2 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">
            <svg
              className="w-4 h-4 mt-0.5 flex-shrink-0"
              fill="currentColor"
              viewBox="0 0 20 20"
              aria-hidden="true"
            >
              <path
                fillRule="evenodd"
                d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
                clipRule="evenodd"
              />
            </svg>
            <span>
              Required field{unmappedRequired.length > 1 ? 's' : ''} not mapped:{' '}
              {unmappedRequired.map((f) => targetFields[f] || f).join(', ')}
            </span>
          </div>
        )}

        {/* Normalization Preview Toggle */}
        {normalizations.length > 0 && (
          <div>
            <button
              type="button"
              onClick={() => setShowNormalization((v) => !v)}
              className="text-sm text-primary-600 hover:text-primary-700 hover:underline flex items-center gap-1"
            >
              <svg
                className={`w-3.5 h-3.5 transition-transform ${showNormalization ? 'rotate-90' : ''}`}
                fill="currentColor"
                viewBox="0 0 20 20"
                aria-hidden="true"
              >
                <path
                  fillRule="evenodd"
                  d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z"
                  clipRule="evenodd"
                />
              </svg>
              {showNormalization ? 'Hide' : 'Show'} header normalization details
            </button>

            {showNormalization && (
              <NormalizationPreview normalizations={normalizations} />
            )}
          </div>
        )}

        {/* Action Buttons */}
        <div className="flex items-center gap-2">
          {normalizations.length > 0 && (
            <button
              type="button"
              onClick={handleAutoDetect}
              className="text-xs text-stone-500 hover:text-stone-700 hover:underline"
            >
              Auto-detect all
            </button>
          )}
          <button
            type="button"
            onClick={handleClearAll}
            className="text-xs text-stone-500 hover:text-stone-700 hover:underline"
          >
            Clear all
          </button>
        </div>
      </div>

      {/* Footer */}
      <div className="px-6 py-4 border-t border-stone-200 bg-white flex items-center justify-end gap-3">
        <button
          type="button"
          onClick={onBack}
          className="px-4 py-2 text-sm font-medium text-stone-700 bg-white border border-stone-300 rounded-lg hover:bg-stone-50 transition-colors"
        >
          Back
        </button>
        <button
          type="button"
          onClick={() => onConfirm(mapping)}
          disabled={!canContinue}
          className={`
            px-4 py-2 text-sm font-medium text-white rounded-lg transition-all
            ${
              canContinue
                ? 'bg-primary-500 hover:bg-primary-600'
                : 'bg-stone-300 cursor-not-allowed'
            }
          `}
        >
          Continue
        </button>
      </div>
    </div>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

function HrisPresetSelector({
  presets,
  selectedPreset,
  onSelect,
}: {
  presets: HrisPreset[];
  selectedPreset: HrisPresetId | null;
  onSelect: (id: HrisPresetId | '') => void;
}) {
  return (
    <div className="flex items-center gap-3 p-3 bg-stone-50 rounded-lg border border-stone-200">
      <label
        htmlFor="hris-preset"
        className="text-sm font-medium text-stone-700 whitespace-nowrap"
      >
        HRIS System:
      </label>
      <select
        id="hris-preset"
        aria-label="HRIS system preset"
        value={selectedPreset ?? ''}
        onChange={(e) =>
          onSelect(e.target.value as HrisPresetId | '')
        }
        className="flex-1 px-3 py-1.5 text-sm border border-stone-300 rounded-lg bg-white focus:outline-none focus:ring-2 focus:ring-primary-200 focus:border-primary-400"
      >
        <option value="">Auto-detect</option>
        {presets.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
    </div>
  );
}

function MappingRow({
  fieldKey,
  fieldLabel,
  isRequired,
  isMapped,
  sourceHeaders,
  usedHeaders,
  currentValue,
  normHint,
  onChange,
}: {
  fieldKey: string;
  fieldLabel: string;
  isRequired: boolean;
  isMapped: boolean;
  sourceHeaders: string[];
  usedHeaders: Set<string>;
  currentValue: string;
  normHint?: HeaderNormalization;
  onChange: (header: string) => void;
}) {
  const selectId = `mapping-${fieldKey}`;

  return (
    <div
      role="row"
      className={`
        grid grid-cols-[1fr_auto_1fr] gap-3 items-center
        px-3 py-2 rounded-lg transition-colors
        ${
          isMapped
            ? 'bg-primary-50/50'
            : isRequired
              ? 'bg-red-50/50'
              : 'hover:bg-stone-50'
        }
      `}
    >
      {/* Target field */}
      <div role="cell" className="flex items-center gap-2">
        <label
          htmlFor={selectId}
          className={`text-sm font-medium ${
            isMapped
              ? 'text-primary-700'
              : isRequired
                ? 'text-red-700'
                : 'text-stone-700'
          }`}
        >
          {fieldLabel}
        </label>
        {isRequired && (
          <span className="text-xs text-red-500" aria-label="required">
            *
          </span>
        )}
      </div>

      {/* Arrow */}
      <div role="cell" className="w-8 flex justify-center" aria-hidden="true">
        {isMapped ? (
          <svg
            className="w-4 h-4 text-primary-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M13 7l5 5m0 0l-5 5m5-5H6"
            />
          </svg>
        ) : (
          <svg
            className="w-4 h-4 text-stone-300"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M13 7l5 5m0 0l-5 5m5-5H6"
            />
          </svg>
        )}
      </div>

      {/* Source column dropdown */}
      <div role="cell">
        <select
          id={selectId}
          value={currentValue}
          onChange={(e) => onChange(e.target.value)}
          className={`
            w-full px-3 py-1.5 text-sm rounded-lg border
            focus:outline-none focus:ring-2 focus:ring-primary-200 focus:border-primary-400
            ${
              isMapped
                ? 'border-primary-300 bg-white text-stone-800'
                : 'border-stone-300 bg-white text-stone-600'
            }
          `}
        >
          <option value="">
            {isRequired ? '-- Select (required) --' : '-- Not mapped --'}
          </option>
          {sourceHeaders.map((header) => (
            <option
              key={header}
              value={header}
              disabled={usedHeaders.has(header) && header !== currentValue}
            >
              {header}
              {usedHeaders.has(header) && header !== currentValue
                ? ' (in use)'
                : ''}
            </option>
          ))}
        </select>
        {normHint && !isMapped && normHint.confidence > 0.3 && (
          <p className="mt-0.5 text-xs text-stone-400">
            Suggestion: "{normHint.original}" ({Math.round(normHint.confidence * 100)}%
            match)
          </p>
        )}
      </div>
    </div>
  );
}

function NormalizationPreview({
  normalizations,
}: {
  normalizations: HeaderNormalization[];
}) {
  return (
    <div className="mt-3 border border-stone-200 rounded-lg overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="bg-stone-50 border-b border-stone-200">
            <th className="px-3 py-2 text-left text-xs font-semibold text-stone-500 uppercase">
              Original
            </th>
            <th className="px-3 py-2 text-left text-xs font-semibold text-stone-500 uppercase">
              Normalized
            </th>
            <th className="px-3 py-2 text-left text-xs font-semibold text-stone-500 uppercase">
              Detected As
            </th>
            <th className="px-3 py-2 text-right text-xs font-semibold text-stone-500 uppercase">
              Confidence
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-stone-100">
          {normalizations.map((norm, i) => (
            <tr key={i} className={i % 2 === 0 ? 'bg-white' : 'bg-stone-50/50'}>
              <td className="px-3 py-1.5 text-stone-700 font-mono text-xs">
                {norm.original}
              </td>
              <td className="px-3 py-1.5 text-stone-500 font-mono text-xs">
                {norm.normalized}
              </td>
              <td className="px-3 py-1.5">
                {norm.detectedField ? (
                  <span className="text-primary-600 font-medium text-xs">
                    {norm.detectedField}
                  </span>
                ) : (
                  <span className="text-stone-400 text-xs italic">none</span>
                )}
              </td>
              <td className="px-3 py-1.5 text-right">
                <span
                  className={`text-xs font-medium ${
                    norm.confidence > 0.7
                      ? 'text-primary-600'
                      : norm.confidence > 0.4
                        ? 'text-amber-600'
                        : 'text-stone-400'
                  }`}
                >
                  {Math.round(norm.confidence * 100)}%
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default ColumnMappingStep;
