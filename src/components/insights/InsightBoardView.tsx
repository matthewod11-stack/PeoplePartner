/**
 * InsightBoardView Component (V2.3.2i-j)
 *
 * Modal view for displaying and managing a single insight board.
 * Shows pinned charts in a responsive grid with unpin/edit capabilities.
 * V2.3.2j: Adds annotation support for notes on pinned charts.
 */

import { useState, useEffect, useCallback } from 'react';
import { Modal } from '../shared/Modal';
import { Button } from '../ui/Button';
import { AnalyticsChart, type DrilldownParams } from '../analytics/AnalyticsChart';
import { ChartAnnotationForm } from './ChartAnnotationForm';
import { ChartAnnotationList } from './ChartAnnotationList';
import { DrilldownModal } from './DrilldownModal';
import { PrintableReport } from './PrintableReport';
import {
  getInsightBoard,
  getChartsForBoard,
  updateInsightBoard,
  unpinChart,
  getAnnotationsForChart,
  createChartAnnotation,
  updateChartAnnotation,
  deleteChartAnnotation,
  type InsightBoard,
  type PinnedChart,
  type ChartAnnotation,
  type EmployeeFilter,
} from '../../lib/tauri-commands';
import { parseChartData, parseAnalyticsRequestFromChart, type AnnotationType } from '../../lib/insight-canvas-types';

interface InsightBoardViewProps {
  /** Board ID to display */
  boardId: string | null;
  /** Called when modal should close */
  onClose: () => void;
}

export function InsightBoardView({ boardId, onClose }: InsightBoardViewProps) {
  const [board, setBoard] = useState<InsightBoard | null>(null);
  const [charts, setCharts] = useState<PinnedChart[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Edit mode for board name
  const [isEditingName, setIsEditingName] = useState(false);
  const [editName, setEditName] = useState('');

  // V2.3.2j: Annotation state
  const [chartAnnotations, setChartAnnotations] = useState<Record<string, ChartAnnotation[]>>({});
  const [expandedAnnotationForm, setExpandedAnnotationForm] = useState<string | null>(null);

  // V2.3.2l: Drilldown state
  const [drilldownFilter, setDrilldownFilter] = useState<EmployeeFilter | null>(null);
  const [drilldownLabel, setDrilldownLabel] = useState<string>('');

  // V2.3.2k: Print mode state
  const [isPrinting, setIsPrinting] = useState(false);

  // Load board, charts, and annotations
  const loadBoard = useCallback(async () => {
    if (!boardId) return;

    setIsLoading(true);
    setError(null);
    try {
      const [boardData, chartsData] = await Promise.all([
        getInsightBoard(boardId),
        getChartsForBoard(boardId),
      ]);
      setBoard(boardData);
      setCharts(chartsData);
      setEditName(boardData.name);

      // V2.3.2j: Load annotations for all charts in parallel
      const annotationsPromises = chartsData.map(async (chart) => {
        const annotations = await getAnnotationsForChart(chart.id);
        return { chartId: chart.id, annotations };
      });
      const annotationsResults = await Promise.all(annotationsPromises);
      const annotationsMap: Record<string, ChartAnnotation[]> = {};
      annotationsResults.forEach(({ chartId, annotations }) => {
        annotationsMap[chartId] = annotations;
      });
      setChartAnnotations(annotationsMap);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load board');
    } finally {
      setIsLoading(false);
    }
  }, [boardId]);

  useEffect(() => {
    if (boardId) {
      loadBoard();
    }
  }, [boardId, loadBoard]);

  // Update board name
  const handleSaveName = useCallback(async () => {
    if (!board || !editName.trim() || editName === board.name) {
      setIsEditingName(false);
      return;
    }

    try {
      const updated = await updateInsightBoard(board.id, { name: editName.trim() });
      setBoard(updated);
      setIsEditingName(false);
    } catch (err) {
      console.error('[InsightBoardView] Rename failed:', err);
    }
  }, [board, editName]);

  // Unpin a chart
  const handleUnpin = useCallback(
    async (chartId: string) => {
      if (!confirm('Remove this chart from the board?')) return;

      try {
        await unpinChart(chartId);
        setCharts((prev) => prev.filter((c) => c.id !== chartId));
        // Also remove annotations from state
        setChartAnnotations((prev) => {
          const next = { ...prev };
          delete next[chartId];
          return next;
        });
      } catch (err) {
        console.error('[InsightBoardView] Unpin failed:', err);
      }
    },
    []
  );

  // V2.3.2j: Create annotation
  const handleCreateAnnotation = useCallback(
    async (chartId: string, content: string, annotationType: AnnotationType) => {
      try {
        const annotation = await createChartAnnotation({
          chart_id: chartId,
          content,
          annotation_type: annotationType,
        });
        setChartAnnotations((prev) => ({
          ...prev,
          [chartId]: [...(prev[chartId] || []), annotation],
        }));
        setExpandedAnnotationForm(null);
      } catch (err) {
        console.error('[InsightBoardView] Create annotation failed:', err);
      }
    },
    []
  );

  // V2.3.2j: Update annotation
  const handleUpdateAnnotation = useCallback(
    async (chartId: string, annotationId: string, content: string) => {
      try {
        const updated = await updateChartAnnotation(annotationId, content);
        setChartAnnotations((prev) => ({
          ...prev,
          [chartId]: (prev[chartId] || []).map((a) =>
            a.id === annotationId ? updated : a
          ),
        }));
      } catch (err) {
        console.error('[InsightBoardView] Update annotation failed:', err);
      }
    },
    []
  );

  // V2.3.2j: Delete annotation
  const handleDeleteAnnotation = useCallback(
    async (chartId: string, annotationId: string) => {
      try {
        await deleteChartAnnotation(annotationId);
        setChartAnnotations((prev) => ({
          ...prev,
          [chartId]: (prev[chartId] || []).filter((a) => a.id !== annotationId),
        }));
      } catch (err) {
        console.error('[InsightBoardView] Delete annotation failed:', err);
      }
    },
    []
  );

  // V2.3.2l: Handle drilldown
  const handleDrilldown = useCallback((params: DrilldownParams) => {
    setDrilldownFilter(params.filter);
    setDrilldownLabel(params.label);
  }, []);

  const closeDrilldown = useCallback(() => {
    setDrilldownFilter(null);
    setDrilldownLabel('');
  }, []);

  // V2.3.2k: Export report handler
  const handleExportReport = useCallback(() => {
    setIsPrinting(true);
    // Small delay to ensure PrintableReport is rendered before print dialog opens
    setTimeout(() => {
      window.print();
      setIsPrinting(false);
    }, 100);
  }, []);

  // Format date
  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  };

  if (!boardId) return null;

  return (
    <Modal isOpen={!!boardId} onClose={onClose} maxWidth="max-w-5xl">
      {isLoading ? (
        <div className="py-12 text-center text-stone-500">Loading board...</div>
      ) : error ? (
        <div className="py-12 text-center text-red-600">{error}</div>
      ) : board ? (
        <div className="space-y-6">
          {/* Board header */}
          <div className="flex items-center justify-between border-b border-stone-200 pb-4">
            <div className="flex-1">
              {isEditingName ? (
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    className="
                      text-xl font-semibold text-stone-900
                      px-2 py-1 -ml-2 rounded-md border border-stone-300
                      focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent
                    "
                    autoFocus
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleSaveName();
                      if (e.key === 'Escape') {
                        setEditName(board.name);
                        setIsEditingName(false);
                      }
                    }}
                    onBlur={handleSaveName}
                  />
                </div>
              ) : (
                <button
                  onClick={() => setIsEditingName(true)}
                  className="
                    text-xl font-semibold text-stone-900
                    hover:text-primary-600 transition-colors
                    flex items-center gap-2 group
                  "
                >
                  {board.name}
                  <svg
                    className="w-4 h-4 text-stone-400 opacity-0 group-hover:opacity-100 transition-opacity"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125"
                    />
                  </svg>
                </button>
              )}
              <p className="text-sm text-stone-500 mt-1">
                {charts.length} {charts.length === 1 ? 'chart' : 'charts'} · Updated{' '}
                {formatDate(board.updated_at)}
              </p>
            </div>
            <div className="flex gap-2">
              {/* V2.3.2k: Export button */}
              {charts.length > 0 && (
                <Button
                  variant="secondary"
                  onClick={handleExportReport}
                  disabled={isPrinting}
                  leftIcon={
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M6.72 13.829c-.24.03-.48.062-.72.096m.72-.096a42.415 42.415 0 0110.56 0m-10.56 0L6.34 18m10.94-4.171c.24.03.48.062.72.096m-.72-.096L17.66 18m0 0l.229 2.523a1.125 1.125 0 01-1.12 1.227H7.231c-.662 0-1.18-.568-1.12-1.227L6.34 18m11.318 0h1.091A2.25 2.25 0 0021 15.75V9.456c0-1.081-.768-2.015-1.837-2.175a48.055 48.055 0 00-1.913-.247M6.34 18H5.25A2.25 2.25 0 013 15.75V9.456c0-1.081.768-2.015 1.837-2.175a48.041 48.041 0 011.913-.247m10.5 0a48.536 48.536 0 00-10.5 0m10.5 0V3.375c0-.621-.504-1.125-1.125-1.125h-8.25c-.621 0-1.125.504-1.125 1.125v3.659M18 10.5h.008v.008H18V10.5zm-3 0h.008v.008H15V10.5z"
                      />
                    </svg>
                  }
                >
                  {isPrinting ? 'Preparing...' : 'Export'}
                </Button>
              )}
              <Button variant="secondary" onClick={onClose}>
                Close
              </Button>
            </div>
          </div>

          {/* Charts grid */}
          {charts.length === 0 ? (
            <div className="py-12 text-center">
              <div className="w-16 h-16 mx-auto mb-4 rounded-xl bg-stone-100 flex items-center justify-center">
                <svg
                  className="w-8 h-8 text-stone-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={1.5}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M3.75 3v11.25A2.25 2.25 0 006 16.5h2.25M3.75 3h-1.5m1.5 0h16.5m0 0h1.5m-1.5 0v11.25A2.25 2.25 0 0118 16.5h-2.25m-7.5 0h7.5m-7.5 0l-1 3m8.5-3l1 3m0 0l.5 1.5m-.5-1.5h-9.5m0 0l-.5 1.5m.75-9l3-3 2.148 2.148A12.061 12.061 0 0116.5 7.605"
                  />
                </svg>
              </div>
              <p className="text-stone-600 font-medium mb-1">No charts pinned</p>
              <p className="text-sm text-stone-500">
                Ask analytics questions in chat and click "Pin" to save charts here.
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {charts.map((pinnedChart) => {
                const chartData = parseChartData(pinnedChart);
                const analyticsRequest = parseAnalyticsRequestFromChart(pinnedChart);
                const annotations = chartAnnotations[pinnedChart.id] || [];

                if (!chartData) return null;

                return (
                  <div key={pinnedChart.id} className="relative group">
                    {/* Action buttons */}
                    <div className="absolute top-2 right-2 z-10 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      {/* Add Note button */}
                      <button
                        onClick={() => setExpandedAnnotationForm(
                          expandedAnnotationForm === pinnedChart.id ? null : pinnedChart.id
                        )}
                        className="
                          p-1.5 rounded-md
                          bg-white/80 backdrop-blur-sm shadow-sm
                          text-stone-400 hover:text-primary-600 hover:bg-primary-50
                          transition-colors
                        "
                        aria-label="Add note"
                      >
                        <svg
                          className="w-4 h-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          strokeWidth={2}
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M7.5 8.25h9m-9 3H12m-9.75 1.51c0 1.6 1.123 2.994 2.707 3.227 1.129.166 2.27.293 3.423.379.35.026.67.21.865.501L12 21l2.755-4.133a1.14 1.14 0 01.865-.501 48.172 48.172 0 003.423-.379c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0012 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018z"
                          />
                        </svg>
                      </button>
                      {/* Unpin button */}
                      <button
                        onClick={() => handleUnpin(pinnedChart.id)}
                        className="
                          p-1.5 rounded-md
                          bg-white/80 backdrop-blur-sm shadow-sm
                          text-stone-400 hover:text-red-600 hover:bg-red-50
                          transition-colors
                        "
                        aria-label="Unpin chart"
                      >
                        <svg
                          className="w-4 h-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          strokeWidth={2}
                        >
                          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    </div>

                    {/* Chart - render without pin button (we're already in the board view) */}
                    <div className="bg-white rounded-xl border border-stone-200/60 shadow-sm overflow-hidden">
                      <AnalyticsChart
                        data={chartData}
                        analyticsRequest={analyticsRequest ?? undefined}
                        onDrilldown={handleDrilldown}
                      />
                      <div className="px-4 pb-3 text-xs text-stone-400">
                        Pinned {formatDate(pinnedChart.pinned_at)}
                        {annotations.length > 0 && (
                          <span className="ml-2">
                            · {annotations.length} {annotations.length === 1 ? 'note' : 'notes'}
                          </span>
                        )}
                      </div>

                      {/* V2.3.2j: Annotation form (expanded) */}
                      {expandedAnnotationForm === pinnedChart.id && (
                        <div className="px-4 pb-4">
                          <ChartAnnotationForm
                            chartId={pinnedChart.id}
                            onSave={(content, type) =>
                              handleCreateAnnotation(pinnedChart.id, content, type)
                            }
                            onCancel={() => setExpandedAnnotationForm(null)}
                          />
                        </div>
                      )}

                      {/* V2.3.2j: Annotation list */}
                      {annotations.length > 0 && (
                        <div className="px-4 pb-4">
                          <ChartAnnotationList
                            chartId={pinnedChart.id}
                            annotations={annotations}
                            onUpdate={(id, content) =>
                              handleUpdateAnnotation(pinnedChart.id, id, content)
                            }
                            onDelete={(id) => handleDeleteAnnotation(pinnedChart.id, id)}
                          />
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      ) : null}

      {/* V2.3.2l: Drilldown modal */}
      {drilldownFilter && (
        <DrilldownModal
          isOpen={!!drilldownFilter}
          onClose={closeDrilldown}
          filter={drilldownFilter}
          label={drilldownLabel}
        />
      )}

      {/* V2.3.2k: Printable report (hidden, shown only during print) */}
      {board && isPrinting && (
        <div className="print-overlay fixed inset-0 bg-white z-[9999] hidden print:block overflow-auto">
          <PrintableReport board={board} charts={charts} annotations={chartAnnotations} />
        </div>
      )}
    </Modal>
  );
}

export default InsightBoardView;
