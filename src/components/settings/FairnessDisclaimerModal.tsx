/**
 * FairnessDisclaimerModal Component (V2.4.2)
 *
 * First-use consent modal for the Fairness Lens feature.
 * Requires users to acknowledge the nature of demographic analysis before enabling.
 */

import { useState, useCallback } from 'react';
import { Modal } from '../shared/Modal';

interface FairnessDisclaimerModalProps {
  /** Whether the modal is open */
  isOpen: boolean;
  /** Called when the modal should close */
  onClose: () => void;
  /** Called when user acknowledges and enables the feature */
  onEnable: () => void;
}

export function FairnessDisclaimerModal({
  isOpen,
  onClose,
  onEnable,
}: FairnessDisclaimerModalProps) {
  const [acknowledged, setAcknowledged] = useState(false);

  const handleEnable = useCallback(() => {
    if (acknowledged) {
      onEnable();
    }
  }, [acknowledged, onEnable]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Enable Fairness Lens"
      maxWidth="max-w-lg"
    >
      <div className="space-y-5">
        {/* What This Feature Does */}
        <div>
          <h4 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-2">
            <svg className="w-4 h-4 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
            What This Feature Does
          </h4>
          <ul className="space-y-2 text-sm text-stone-600">
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Shows <strong>demographic representation</strong> breakdown by gender and ethnicity
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Compares <strong>average performance ratings</strong> across demographic groups
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Analyzes <strong>promotion rate patterns</strong> (inferred from job titles)
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Suppresses groups with <strong>fewer than 5 members</strong> to protect privacy
              </span>
            </li>
          </ul>
        </div>

        {/* What This Feature Does NOT Do */}
        <div>
          <h4 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-2">
            <svg className="w-4 h-4 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
            </svg>
            What This Feature Does NOT Do
          </h4>
          <ul className="space-y-2 text-sm text-stone-600">
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Provide <strong>individual-level</strong> demographic analysis or predictions
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Account for <strong>contextual factors</strong> like role type, location, or tenure
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Replace proper <strong>pay equity audits</strong> or compliance reviews
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Provide <strong>legally defensible</strong> DEI metrics for compliance purposes
              </span>
            </li>
          </ul>
        </div>

        {/* Acknowledgment */}
        <div className="p-4 bg-primary-50 border border-primary-200 rounded-lg">
          <label className="flex gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(e) => setAcknowledged(e.target.checked)}
              className="w-5 h-5 mt-0.5 rounded border-stone-300 text-primary-500 focus:ring-primary-500"
            />
            <span className="text-sm text-primary-900">
              I understand these are <strong>exploratory metrics</strong> that may reveal
              systemic patterns rather than individual differences. I will use them as
              conversation starters for organizational improvement, not as the basis for
              employment decisions.
            </span>
          </label>
        </div>

        {/* Actions */}
        <div className="flex justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleEnable}
            disabled={!acknowledged}
            className={`
              px-4 py-2 text-sm font-medium rounded-lg transition-colors
              ${
                acknowledged
                  ? 'bg-primary-500 text-white hover:bg-primary-600'
                  : 'bg-stone-200 text-stone-400 cursor-not-allowed'
              }
            `}
          >
            Enable Fairness Lens
          </button>
        </div>
      </div>
    </Modal>
  );
}

export default FairnessDisclaimerModal;
