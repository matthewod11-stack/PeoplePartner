// People Partner - Persona Tile Selector (Onboarding)
// Visual tile-based persona selector for the onboarding company step

import { useState, useEffect, useCallback } from 'react';
import {
  getPersonas,
  getSetting,
  setSetting,
  type Persona,
} from '../../../lib/tauri-commands';

// Emoji/icon mapping for each persona
const PERSONA_ICONS: Record<string, string> = {
  alex: '\u{1F464}',    // 👤 Silhouette (neutral, professional)
  jordan: '\u{1F4CB}',  // 📋 Clipboard (compliance focus)
  sam: '\u{1F680}',     // 🚀 Rocket (startup energy)
  morgan: '\u{1F4CA}',  // 📊 Chart (data-driven)
  taylor: '\u{1F49A}',  // 💚 Green heart (employee-first)
};

// Short 2-word style descriptions for tiles
const PERSONA_SHORT_STYLE: Record<string, string> = {
  alex: 'Warm & practical',
  jordan: 'Compliance-first',
  sam: 'Startup-direct',
  morgan: 'Data-driven',
  taylor: 'Employee-first',
};

interface PersonaTileSelectorProps {
  /** Called when a persona is selected (useful for parent tracking) */
  onSelect?: (personaId: string) => void;
}

export function PersonaTileSelector({ onSelect }: PersonaTileSelectorProps) {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [selectedId, setSelectedId] = useState<string>('alex');
  const [loading, setLoading] = useState(true);

  // Load personas and current selection on mount
  useEffect(() => {
    Promise.all([getPersonas(), getSetting('persona')])
      .then(([personaList, currentPersona]) => {
        setPersonas(personaList);
        setSelectedId(currentPersona || 'alex');
        setLoading(false);
      })
      .catch(() => {
        setLoading(false);
      });
  }, []);

  const handleSelect = useCallback(
    async (id: string) => {
      const previousId = selectedId;
      setSelectedId(id);
      try {
        await setSetting('persona', id);
        onSelect?.(id);
      } catch {
        // Revert on error
        setSelectedId(previousId);
      }
    },
    [selectedId, onSelect]
  );

  if (loading) {
    return (
      <div className="grid grid-cols-2 gap-3">
        {[1, 2, 3, 4, 5].map((i) => (
          <div
            key={i}
            className={`h-20 bg-stone-100 rounded-xl animate-pulse ${i === 5 ? 'col-span-2 max-w-[calc(50%-6px)] mx-auto' : ''}`}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="mt-8">
      {/* Section header */}
      <h3 className="text-sm font-medium text-stone-700 mb-3">
        Choose your AI advisor style
      </h3>
      <p className="text-xs text-stone-500 mb-4">
        This affects how your HR assistant communicates. You can change it anytime in Settings.
      </p>

      {/* Tile grid */}
      <div className="grid grid-cols-2 gap-3">
        {personas.map((persona, index) => {
          const isSelected = selectedId === persona.id;
          const icon = PERSONA_ICONS[persona.id] || '\u{1F464}';
          const shortStyle = PERSONA_SHORT_STYLE[persona.id] || persona.style;
          const isLastOdd = index === personas.length - 1 && personas.length % 2 === 1;

          return (
            <button
              key={persona.id}
              type="button"
              onClick={() => handleSelect(persona.id)}
              className={`
                relative p-4 rounded-xl border-2 transition-all duration-200
                ${isSelected
                  ? 'border-primary-500 bg-primary-50 shadow-sm'
                  : 'border-stone-200 bg-white hover:border-stone-300 hover:bg-stone-50'
                }
                ${isLastOdd ? 'col-span-2 max-w-[calc(50%-6px)] mx-auto w-full' : ''}
              `}
            >
              {/* Selected checkmark */}
              {isSelected && (
                <div className="absolute top-2 right-2">
                  <svg
                    className="w-5 h-5 text-primary-600"
                    fill="currentColor"
                    viewBox="0 0 20 20"
                  >
                    <path
                      fillRule="evenodd"
                      d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                      clipRule="evenodd"
                    />
                  </svg>
                </div>
              )}

              {/* Icon */}
              <div className="text-2xl mb-1">{icon}</div>

              {/* Name */}
              <div className="font-medium text-stone-800 text-sm">
                {persona.name}
              </div>

              {/* Short style */}
              <div className="text-xs text-stone-500 mt-0.5">
                {shortStyle}
              </div>

              {/* Default indicator for Alex */}
              {persona.id === 'alex' && !isSelected && (
                <div className="mt-1 text-[10px] text-stone-400 uppercase tracking-wide">
                  Default
                </div>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export default PersonaTileSelector;
