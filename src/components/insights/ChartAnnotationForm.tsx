/**
 * ChartAnnotationForm Component (V2.3.2j)
 *
 * Inline form for adding/editing annotations on pinned charts.
 * Supports three annotation types: note, callout, question.
 */

import { useState, useCallback } from 'react';
import { Button } from '../ui/Button';
import type { AnnotationType, ChartAnnotation } from '../../lib/insight-canvas-types';

interface ChartAnnotationFormProps {
  /** Chart ID to annotate */
  chartId: string;
  /** Existing annotation for edit mode */
  annotation?: ChartAnnotation;
  /** Called when form is submitted */
  onSave: (content: string, annotationType: AnnotationType) => Promise<void>;
  /** Called when form is cancelled */
  onCancel: () => void;
  /** Whether save is in progress */
  isSaving?: boolean;
}

const ANNOTATION_TYPES: { type: AnnotationType; label: string; icon: string }[] = [
  { type: 'note', label: 'Note', icon: '📝' },
  { type: 'callout', label: 'Callout', icon: '💡' },
  { type: 'question', label: 'Question', icon: '❓' },
];

export function ChartAnnotationForm({
  annotation,
  onSave,
  onCancel,
  isSaving = false,
}: ChartAnnotationFormProps) {
  const [content, setContent] = useState(annotation?.content ?? '');
  const [annotationType, setAnnotationType] = useState<AnnotationType>(
    annotation?.annotation_type ?? 'note'
  );

  const isEditMode = !!annotation;

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!content.trim()) return;
      await onSave(content.trim(), annotationType);
    },
    [content, annotationType, onSave]
  );

  return (
    <form onSubmit={handleSubmit} className="mt-3 p-3 bg-stone-50 rounded-lg border border-stone-200">
      {/* Type selector */}
      <div className="flex gap-2 mb-3">
        {ANNOTATION_TYPES.map(({ type, label, icon }) => (
          <button
            key={type}
            type="button"
            onClick={() => setAnnotationType(type)}
            className={`
              flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium
              transition-colors
              ${
                annotationType === type
                  ? type === 'note'
                    ? 'bg-stone-200 text-stone-700'
                    : type === 'callout'
                      ? 'bg-amber-100 text-amber-700'
                      : 'bg-primary-100 text-primary-700'
                  : 'bg-white text-stone-500 hover:bg-stone-100 border border-stone-200'
              }
            `}
          >
            <span>{icon}</span>
            {label}
          </button>
        ))}
      </div>

      {/* Content textarea */}
      <textarea
        value={content}
        onChange={(e) => setContent(e.target.value)}
        placeholder={
          annotationType === 'question'
            ? 'What question does this data raise?'
            : annotationType === 'callout'
              ? 'What insight stands out?'
              : 'Add your note...'
        }
        className="
          w-full px-3 py-2 rounded-md border border-stone-300
          text-sm text-stone-700 placeholder:text-stone-400
          focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent
          resize-none
        "
        rows={3}
        autoFocus
      />

      {/* Action buttons */}
      <div className="flex justify-end gap-2 mt-3">
        <Button type="button" variant="secondary" size="sm" onClick={onCancel} disabled={isSaving}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" size="sm" disabled={!content.trim() || isSaving}>
          {isSaving ? 'Saving...' : isEditMode ? 'Update' : 'Save'}
        </Button>
      </div>
    </form>
  );
}

export default ChartAnnotationForm;
