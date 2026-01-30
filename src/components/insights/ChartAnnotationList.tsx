/**
 * ChartAnnotationList Component (V2.3.2j)
 *
 * Displays annotations below a pinned chart with type badges,
 * edit/delete actions, and empty state.
 */

import { useState } from 'react';
import type { ChartAnnotation, AnnotationType } from '../../lib/insight-canvas-types';
import { ChartAnnotationForm } from './ChartAnnotationForm';

interface ChartAnnotationListProps {
  /** Chart ID */
  chartId: string;
  /** List of annotations */
  annotations: ChartAnnotation[];
  /** Called when updating an annotation */
  onUpdate: (id: string, content: string) => Promise<void>;
  /** Called when deleting an annotation */
  onDelete: (id: string) => Promise<void>;
}

const TYPE_STYLES: Record<AnnotationType, { bg: string; text: string; icon: string }> = {
  note: { bg: 'bg-stone-100', text: 'text-stone-600', icon: '📝' },
  callout: { bg: 'bg-amber-100', text: 'text-amber-700', icon: '💡' },
  question: { bg: 'bg-primary-100', text: 'text-primary-700', icon: '❓' },
};

export function ChartAnnotationList({
  chartId,
  annotations,
  onUpdate,
  onDelete,
}: ChartAnnotationListProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);

  const handleUpdate = async (content: string) => {
    if (!editingId) return;
    setIsUpdating(true);
    try {
      await onUpdate(editingId, content);
      setEditingId(null);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this annotation?')) return;
    await onDelete(id);
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    });
  };

  if (annotations.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 space-y-2">
      {annotations.map((annotation) => {
        const style = TYPE_STYLES[annotation.annotation_type];

        // Edit mode
        if (editingId === annotation.id) {
          return (
            <ChartAnnotationForm
              key={annotation.id}
              chartId={chartId}
              annotation={annotation}
              onSave={handleUpdate}
              onCancel={() => setEditingId(null)}
              isSaving={isUpdating}
            />
          );
        }

        // Display mode
        return (
          <div
            key={annotation.id}
            className="group flex items-start gap-3 p-3 bg-white rounded-lg border border-stone-200"
          >
            {/* Type badge */}
            <span
              className={`
                inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-medium shrink-0
                ${style.bg} ${style.text}
              `}
            >
              <span>{style.icon}</span>
              {annotation.annotation_type.charAt(0).toUpperCase() + annotation.annotation_type.slice(1)}
            </span>

            {/* Content */}
            <div className="flex-1 min-w-0">
              <p className="text-sm text-stone-700 whitespace-pre-wrap">{annotation.content}</p>
              <p className="text-xs text-stone-400 mt-1">{formatDate(annotation.created_at)}</p>
            </div>

            {/* Actions */}
            <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
              <button
                onClick={() => setEditingId(annotation.id)}
                className="p-1.5 rounded-md text-stone-400 hover:text-stone-600 hover:bg-stone-100"
                aria-label="Edit annotation"
              >
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125"
                  />
                </svg>
              </button>
              <button
                onClick={() => handleDelete(annotation.id)}
                className="p-1.5 rounded-md text-stone-400 hover:text-red-600 hover:bg-red-50"
                aria-label="Delete annotation"
              >
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export default ChartAnnotationList;
