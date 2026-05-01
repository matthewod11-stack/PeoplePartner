/**
 * UpdateModal Component
 *
 * Shown when the user clicks the header "Update Available" chip. Surfaces
 * the new version + release notes and gives the user explicit Restart /
 * Later choices, instead of installing on click without warning.
 *
 * "Later" persists for 24h via `useUpdateCheck`'s snooze record so the chip
 * doesn't keep nagging on every launch.
 */

import type { Update } from '@tauri-apps/plugin-updater';
import { Modal } from './Modal';

interface UpdateModalProps {
  /** Whether the modal is open */
  isOpen: boolean;
  /** The pending update (carries version + release notes). */
  update: Update | null;
  /** True while download/install/relaunch is in progress. */
  installing: boolean;
  /** Called when user clicks "Restart to install". */
  onInstall: () => void;
  /** Called when user clicks "Later" — defers for 24h. */
  onLater: () => void;
  /** Called on Escape, backdrop click, or close button — same as Later. */
  onClose: () => void;
}

export function UpdateModal({
  isOpen,
  update,
  installing,
  onInstall,
  onLater,
  onClose,
}: UpdateModalProps) {
  // Defensive: parent should not open the modal without an update; render
  // nothing rather than crashing if the contract slips.
  if (!update) return null;

  const releaseNotes = update.body?.trim() ?? '';

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={`Update available: v${update.version}`}
      maxWidth="max-w-xl"
    >
      <div className="space-y-4">
        {releaseNotes ? (
          <div>
            <h3 className="text-sm font-medium text-stone-700 mb-2">
              What's new
            </h3>
            <div
              className="
                text-sm text-stone-600
                bg-stone-50 border border-stone-200 rounded-md
                px-3 py-2
                max-h-72 overflow-y-auto
                whitespace-pre-wrap font-mono
              "
            >
              {releaseNotes}
            </div>
          </div>
        ) : (
          <p className="text-sm text-stone-600">
            Release notes are not available for this version.
          </p>
        )}

        <p className="text-xs text-stone-500">
          Installing will download the update and relaunch People Partner.
          Your data and license stay intact.
        </p>

        <div className="flex justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onLater}
            disabled={installing}
            className="
              px-3 py-1.5
              text-sm font-medium
              text-stone-700
              bg-stone-100 hover:bg-stone-200
              rounded-md
              transition-colors duration-150
              disabled:opacity-50 disabled:cursor-not-allowed
            "
          >
            Later
          </button>
          <button
            type="button"
            onClick={onInstall}
            disabled={installing}
            className="
              px-3 py-1.5
              text-sm font-medium
              text-white
              bg-teal-700 hover:bg-teal-800
              rounded-md
              transition-colors duration-150
              disabled:opacity-70 disabled:cursor-wait
            "
          >
            {installing ? 'Installing…' : 'Restart to install'}
          </button>
        </div>
      </div>
    </Modal>
  );
}

export default UpdateModal;
