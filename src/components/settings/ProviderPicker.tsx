// People Partner - Provider Picker Component
// Shared between onboarding (step 2) and settings panel

import { PROVIDER_META, PROVIDER_ORDER } from '../../lib/provider-config';

interface ProviderPickerProps {
  selectedId: string;
  onSelect: (providerId: string) => void;
  /** Which providers have API keys stored (for badge display) */
  keyStatus?: Record<string, boolean>;
  /** Smaller layout for settings panel */
  compact?: boolean;
}

export function ProviderPicker({
  selectedId,
  onSelect,
  keyStatus,
  compact = false,
}: ProviderPickerProps) {
  return (
    <div className={compact ? 'space-y-2' : 'space-y-3'}>
      {PROVIDER_ORDER.map((id) => {
        const meta = PROVIDER_META[id];
        const isSelected = selectedId === id;
        const hasKey = keyStatus?.[id];

        return (
          <button
            key={id}
            type="button"
            onClick={() => onSelect(id)}
            aria-pressed={isSelected}
            aria-label={`Select ${meta.displayName} as AI provider`}
            className={`
              relative w-full text-left rounded-xl border-2 transition-all duration-200
              ${compact ? 'p-3' : 'p-4'}
              ${
                isSelected
                  ? `${meta.selectedBorder} ${meta.selectedBg}`
                  : 'border-stone-200 bg-white hover:border-stone-300 hover:shadow-sm'
              }
            `}
          >
            {/* Selected checkmark */}
            {isSelected && (
              <div className="absolute top-2.5 right-2.5">
                <div className={`w-5 h-5 rounded-full flex items-center justify-center ${meta.bgColor}`}>
                  <svg
                    className={`w-3.5 h-3.5 ${meta.iconColor}`}
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={3}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                  </svg>
                </div>
              </div>
            )}

            <div className="flex items-center gap-3">
              {/* Provider icon dot */}
              <div className={`w-8 h-8 rounded-full flex items-center justify-center ${meta.bgColor}`}>
                <span className={`text-sm font-bold ${meta.iconColor}`}>
                  {meta.displayName.charAt(0)}
                </span>
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-stone-800">
                    {meta.displayName}
                  </span>
                  <span className="text-xs text-stone-400">
                    {meta.modelName}
                  </span>
                  {/* Key status badge */}
                  {hasKey && (
                    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium rounded-full bg-green-100 text-green-700">
                      <svg className="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                      </svg>
                      Key saved
                    </span>
                  )}
                </div>
                {!compact && (
                  <p className="text-xs text-stone-500 mt-0.5">
                    {meta.description}
                  </p>
                )}
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}

export default ProviderPicker;
