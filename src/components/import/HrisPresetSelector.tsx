import type { HrisPreset, HrisPresetId } from '../../lib/types';

interface HrisPresetSelectorProps {
  presets: HrisPreset[];
  selectedPreset: HrisPresetId | null;
  onSelect: (presetId: HrisPresetId | null) => void;
  disabled?: boolean;
}

export function HrisPresetSelector({
  presets,
  selectedPreset,
  onSelect,
  disabled,
}: HrisPresetSelectorProps) {
  return (
    <div className="flex items-center gap-3">
      <label
        htmlFor="hris-preset"
        className="text-sm font-medium text-stone-700 whitespace-nowrap"
      >
        HRIS System:
      </label>
      <select
        id="hris-preset"
        value={selectedPreset ?? ''}
        onChange={(e) => onSelect(e.target.value ? (e.target.value as HrisPresetId) : null)}
        disabled={disabled}
        className="flex-1 px-3 py-1.5 text-sm border border-stone-300 rounded-lg bg-white
          focus:outline-none focus:ring-2 focus:ring-primary-500/30 focus:border-primary-500
          disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <option value="">Auto-detect (generic)</option>
        {presets.map((preset) => (
          <option key={preset.id} value={preset.id}>
            {preset.name} — {preset.description}
          </option>
        ))}
      </select>
    </div>
  );
}
