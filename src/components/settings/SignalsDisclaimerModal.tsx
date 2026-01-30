/**
 * SignalsDisclaimerModal Component (V2.4.1)
 *
 * First-use consent modal for the Attention Signals feature.
 * Requires users to acknowledge the heuristic nature before enabling.
 */

import { useState, useCallback } from 'react';
import { Modal } from '../shared/Modal';

interface SignalsDisclaimerModalProps {
  /** Whether the modal is open */
  isOpen: boolean;
  /** Called when the modal should close */
  onClose: () => void;
  /** Called when user acknowledges and enables the feature */
  onEnable: () => void;
}

export function SignalsDisclaimerModal({
  isOpen,
  onClose,
  onEnable,
}: SignalsDisclaimerModalProps) {
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
      title="Enable Attention Signals"
      maxWidth="max-w-lg"
    >
      <div className="space-y-5">
        {/* What This Feature Does */}
        <div>
          <h4 className="flex items-center gap-2 text-sm font-medium text-stone-700 mb-2">
            <svg className="w-4 h-4 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            What This Feature Does
          </h4>
          <ul className="space-y-2 text-sm text-stone-600">
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Identifies <strong>team-level patterns</strong> in tenure, performance ratings, and eNPS scores
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Aggregates data at the <strong>department level</strong> (minimum 5 employees)
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Highlights common themes from performance reviews
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-green-500 flex-shrink-0">&#10003;</span>
              <span>
                Provides <strong>conversation starters</strong> for proactive team discussions
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
                Predict which <strong>individuals</strong> might leave — no individual-level signals
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Replace proper 1-on-1 conversations or performance management
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Provide <strong>definitive conclusions</strong> — these are heuristic indicators only
              </span>
            </li>
            <li className="flex gap-2">
              <span className="text-red-500 flex-shrink-0">&#10007;</span>
              <span>
                Account for context that only humans understand
              </span>
            </li>
          </ul>
        </div>

        {/* Acknowledgment */}
        <div className="p-4 bg-amber-50 border border-amber-200 rounded-lg">
          <label className="flex gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(e) => setAcknowledged(e.target.checked)}
              className="w-5 h-5 mt-0.5 rounded border-stone-300 text-primary-500 focus:ring-primary-500"
            />
            <span className="text-sm text-amber-900">
              I understand these are <strong>heuristic indicators</strong> based on aggregate
              patterns, not predictions about individuals. I will use them as conversation
              starters, not as the basis for employment decisions.
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
            Enable Attention Signals
          </button>
        </div>
      </div>
    </Modal>
  );
}

export default SignalsDisclaimerModal;
