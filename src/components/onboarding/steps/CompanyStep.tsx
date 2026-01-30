// HR Command Center - Company Step (Step 3)
// Wraps existing CompanySetup component for onboarding flow
// Includes persona tile selector for AI advisor style

import { useEffect, useState } from 'react';
import { CompanySetup } from '../../company/CompanySetup';
import { hasCompany } from '../../../lib/tauri-commands';
import { PersonaTileSelector } from './PersonaTileSelector';

interface CompanyStepProps {
  onComplete: () => void;
  onValidChange: (valid: boolean) => void;
}

export function CompanyStep({ onComplete, onValidChange }: CompanyStepProps) {
  const [hasExisting, setHasExisting] = useState(false);

  // Check if company already exists on mount
  useEffect(() => {
    hasCompany().then((exists) => {
      setHasExisting(exists);
      onValidChange(exists);
    }).catch(() => {
      // Ignore
    });
  }, [onValidChange]);

  const handleSave = () => {
    setHasExisting(true);
    onValidChange(true);
    // Auto-advance to next step
    onComplete();
  };

  return (
    <div className="w-full">
      {/* Company Setup Form */}
      <CompanySetup
        onSave={handleSave}
        compact={false}
      />

      {/* Persona Tile Selector */}
      <PersonaTileSelector />

      {/* Already configured - show continue button */}
      {hasExisting && (
        <div className="mt-6 text-center">
          <button
            type="button"
            onClick={onComplete}
            className="px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200"
          >
            Continue
          </button>
        </div>
      )}
    </div>
  );
}

export default CompanyStep;
