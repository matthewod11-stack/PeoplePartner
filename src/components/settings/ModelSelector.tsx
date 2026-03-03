// HR Command Center - Model Selector Component
// Lets users choose which AI model to use for the active provider

import { useState, useEffect, useCallback } from 'react';
import type { ModelInfo, ModelTier } from '../../lib/types';
import { getModelsForProvider, getActiveModel, setActiveModel } from '../../lib/tauri-commands';

interface ModelSelectorProps {
  /** The currently active provider ID */
  providerId: string;
  /** Whether in trial mode (disables selection) */
  disabled?: boolean;
}

const TIER_STYLES: Record<ModelTier, { label: string; bg: string; text: string }> = {
  Recommended: { label: 'Recommended', bg: 'bg-green-100', text: 'text-green-700' },
  Premium: { label: 'Premium', bg: 'bg-purple-100', text: 'text-purple-700' },
  Fast: { label: 'Fast', bg: 'bg-blue-100', text: 'text-blue-700' },
};

export function ModelSelector({ providerId, disabled = false }: ModelSelectorProps) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const loadModels = useCallback(async () => {
    setLoading(true);
    try {
      const [modelList, currentModel] = await Promise.all([
        getModelsForProvider(providerId),
        getActiveModel(providerId),
      ]);
      setModels(modelList);
      setActiveModelId(currentModel);
    } catch (err) {
      console.error('Failed to load models:', err);
    } finally {
      setLoading(false);
    }
  }, [providerId]);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const handleSelect = async (modelId: string) => {
    if (disabled) return;
    try {
      await setActiveModel(providerId, modelId);
      setActiveModelId(modelId);
    } catch (err) {
      console.error('Failed to set active model:', err);
    }
  };

  // Determine which model is effectively selected (explicit selection or default)
  const effectiveModelId = activeModelId ?? models.find(m => m.is_default)?.id ?? '';

  if (loading) {
    return (
      <div className="p-3 bg-stone-50 border border-stone-200 rounded-xl">
        <p className="text-xs text-stone-400 text-center">Loading models...</p>
      </div>
    );
  }

  if (models.length === 0) return null;

  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-stone-500 ml-1">Model</p>
      <div className="space-y-1.5">
        {models.map((model) => {
          const isSelected = effectiveModelId === model.id;
          const tierStyle = TIER_STYLES[model.tier];

          return (
            <button
              key={model.id}
              type="button"
              onClick={() => handleSelect(model.id)}
              disabled={disabled}
              aria-pressed={isSelected}
              aria-label={`Select ${model.display_name}`}
              className={`
                relative w-full text-left rounded-lg border transition-all duration-150 p-2.5
                ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
                ${
                  isSelected
                    ? 'border-stone-400 bg-stone-50 shadow-sm'
                    : 'border-stone-200 bg-white hover:border-stone-300'
                }
              `}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                  {/* Radio indicator */}
                  <div
                    className={`
                      w-3.5 h-3.5 rounded-full border-2 flex-shrink-0 flex items-center justify-center
                      ${isSelected ? 'border-stone-600' : 'border-stone-300'}
                    `}
                  >
                    {isSelected && (
                      <div className="w-1.5 h-1.5 rounded-full bg-stone-600" />
                    )}
                  </div>
                  <span className={`text-sm truncate ${isSelected ? 'font-medium text-stone-800' : 'text-stone-600'}`}>
                    {model.display_name}
                  </span>
                </div>
                <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full flex-shrink-0 ${tierStyle.bg} ${tierStyle.text}`}>
                  {tierStyle.label}
                </span>
              </div>
            </button>
          );
        })}
      </div>
      {disabled && (
        <p className="text-[11px] text-stone-400 ml-1">
          Model selection is available in paid mode.
        </p>
      )}
    </div>
  );
}
