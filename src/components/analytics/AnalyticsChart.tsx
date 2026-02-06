/**
 * AnalyticsChart Component (V2.3.2)
 *
 * Renders chart data from analytics requests using Recharts.
 * Supports Bar, Pie, Line, and HorizontalBar chart types.
 * V2.3.2h: Adds "Pin to Canvas" for persistent chart storage.
 * V2.3.2l: Adds drilldown click handlers for chart segments.
 */

import { useState, useCallback, useMemo, useId } from 'react';
import {
  BarChart,
  Bar,
  PieChart,
  Pie,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  Legend,
  Cell,
  ResponsiveContainer,
  CartesianGrid,
} from 'recharts';
import type { ChartData, ChartType, AnalyticsRequest } from '../../lib/analytics-types';
import type { EmployeeFilter } from '../../lib/tauri-commands';
import { FilterCaption } from './FilterCaption';
import { BoardSelectorModal } from './BoardSelectorModal';
import { Button } from '../ui/Button';
import { useConversationMeta } from '../../contexts/ConversationContext';
import { pinChart } from '../../lib/tauri-commands';
import { createPinChartInput } from '../../lib/insight-canvas-types';
import { buildEmployeeFilter, isDrilldownSupported } from '../../lib/drilldown-utils';

// Design tokens matching Tailwind config
const CHART_COLORS = [
  '#7c3aed', // primary-600
  '#a78bfa', // primary-400
  '#c4b5fd', // primary-300
  '#8b5cf6', // primary-500
  '#78716c', // stone-500
  '#a8a29e', // stone-400
  '#d6d3d1', // stone-300
  '#57534e', // stone-600
];

/** V2.3.2l: Drilldown callback parameters */
export interface DrilldownParams {
  filter: EmployeeFilter;
  label: string;
}

interface AnalyticsChartProps {
  data: ChartData;
  /** V2.3.2h: Analytics request for pinning */
  analyticsRequest?: AnalyticsRequest;
  /** V2.3.2h: Message ID for pinning */
  messageId?: string;
  /** V2.3.2l: Called when a chart segment is clicked for drilldown */
  onDrilldown?: (params: DrilldownParams) => void;
}

export function AnalyticsChart({
  data,
  analyticsRequest,
  messageId,
  onDrilldown,
}: AnalyticsChartProps) {
  const { conversationId } = useConversationMeta();
  const chartSummaryId = useId();
  const chartTableId = useId();

  // Modal and pin state
  const [showBoardModal, setShowBoardModal] = useState(false);
  const [isPinning, setIsPinning] = useState(false);
  const [pinSuccess, setPinSuccess] = useState<string | null>(null);

  // V2.3.2l: Drilldown support
  const groupBy = analyticsRequest?.group_by;
  const canDrilldown = onDrilldown && groupBy && isDrilldownSupported(groupBy);

  const handleDrilldown = useCallback(
    (label: string) => {
      if (!onDrilldown || !groupBy) return;

      const result = buildEmployeeFilter(groupBy, label);
      if (result.type === 'filter') {
        onDrilldown({ filter: result.filter, label: result.label });
      }
    },
    [onDrilldown, groupBy]
  );

  // Transform data for Recharts (needs 'name' key for labels)
  const chartData = useMemo(() => data.data.map((point) => ({
    name: point.label,
    value: point.value,
    percentage: point.percentage ?? 0,
  })), [data.data]);

  const chartSummary = useMemo(() => {
    if (chartData.length === 0) {
      return `${data.title}. No chart points available.`;
    }

    const topPoint = chartData.reduce(
      (max, point) => (point.value > max.value ? point : max),
      chartData[0]
    );
    return `${data.title}. ${chartData.length} data point${chartData.length === 1 ? '' : 's'}. Highest value: ${topPoint.name} at ${topPoint.value}.`;
  }, [chartData, data.title]);

  // Handle pin to board
  const handlePinToBoard = useCallback(
    async (boardId: string, boardName: string) => {
      if (!analyticsRequest) {
        console.warn('[AnalyticsChart] No analytics request available for pinning');
        return;
      }

      setIsPinning(true);
      try {
        const input = createPinChartInput(
          boardId,
          data,
          analyticsRequest,
          conversationId,
          messageId
        );
        await pinChart(input);
        setPinSuccess(boardName);
        // Auto-clear success message after 3 seconds
        setTimeout(() => setPinSuccess(null), 3000);
        console.log('[AnalyticsChart] Chart pinned to board:', boardName);
      } catch (err) {
        console.error('[AnalyticsChart] Failed to pin chart:', err);
      } finally {
        setIsPinning(false);
      }
    },
    [data, analyticsRequest, conversationId, messageId]
  );

  // Only show pin button if we have the analytics request
  const canPin = !!analyticsRequest;

  return (
    <div className="mt-4 p-4 bg-white rounded-xl border border-stone-200/60 shadow-sm">
      {/* Header with title and pin button */}
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-sm font-semibold text-stone-900">{data.title}</h4>

        {canPin && (
          <div className="flex items-center gap-2">
            {pinSuccess && (
              <span className="text-xs text-green-600 animate-fade-in">
                Pinned to {pinSuccess}
              </span>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowBoardModal(true)}
              disabled={isPinning}
              leftIcon={
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                  aria-hidden="true"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"
                  />
                </svg>
              }
            >
              {isPinning ? 'Pinning...' : 'Pin'}
            </Button>
          </div>
        )}
      </div>

      <p id={chartSummaryId} className="sr-only">{chartSummary}</p>

      <div className="h-64" role="img" aria-labelledby={chartSummaryId}>
        <ResponsiveContainer width="100%" height="100%">
          {renderChart(data.chart_type, chartData, data, canDrilldown, handleDrilldown)}
        </ResponsiveContainer>
      </div>

      <table id={chartTableId} className="sr-only">
        <caption>{data.title} data table</caption>
        <thead>
          <tr>
            <th>Label</th>
            <th>Value</th>
            <th>Percentage</th>
          </tr>
        </thead>
        <tbody>
          {chartData.map((point) => (
            <tr key={point.name}>
              <td>{point.name}</td>
              <td>{point.value}</td>
              <td>{point.percentage}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <FilterCaption filters={data.filters_applied} total={data.total} />

      {/* Board selector modal */}
      <BoardSelectorModal
        isOpen={showBoardModal}
        onClose={() => setShowBoardModal(false)}
        onSelect={handlePinToBoard}
        chartTitle={data.title}
      />
    </div>
  );
}

interface ChartDataItem {
  name: string;
  value: number;
  percentage: number;
  [key: string]: string | number; // Index signature for Recharts compatibility
}

function renderChart(
  chartType: ChartType,
  chartData: ChartDataItem[],
  data: ChartData,
  canDrilldown?: boolean,
  onDrilldown?: (label: string) => void
) {
  switch (chartType) {
    case 'pie':
      return renderPieChart(chartData, canDrilldown, onDrilldown);

    case 'bar':
      return renderBarChart(chartData, data, 'vertical', canDrilldown, onDrilldown);

    case 'horizontal_bar':
      return renderBarChart(chartData, data, 'horizontal', canDrilldown, onDrilldown);

    case 'line':
      return renderLineChart(chartData, data);

    default:
      return renderBarChart(chartData, data, 'vertical', canDrilldown, onDrilldown);
  }
}

function renderPieChart(
  chartData: ChartDataItem[],
  canDrilldown?: boolean,
  onDrilldown?: (label: string) => void
) {
  return (
    <PieChart>
      <Pie
        data={chartData}
        dataKey="value"
        nameKey="name"
        cx="50%"
        cy="50%"
        outerRadius={80}
        label={({ name, payload }) => `${name} (${payload?.percentage ?? 0}%)`}
        labelLine={{ stroke: '#78716c', strokeWidth: 1 }}
        onClick={canDrilldown ? (_, index) => onDrilldown?.(chartData[index].name) : undefined}
        style={canDrilldown ? { cursor: 'pointer' } : undefined}
      >
        {chartData.map((_, index) => (
          <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
        ))}
      </Pie>
      <Tooltip
        formatter={(value) => {
          const numValue = typeof value === 'number' ? value : 0;
          return [`${numValue}`, 'Count'];
        }}
        contentStyle={{
          backgroundColor: 'white',
          border: '1px solid #e7e5e4',
          borderRadius: '8px',
          fontSize: '12px',
        }}
      />
      <Legend
        wrapperStyle={{ fontSize: '12px' }}
        formatter={(value) => <span className="text-stone-700">{value}</span>}
      />
    </PieChart>
  );
}

function renderBarChart(
  chartData: ChartDataItem[],
  data: ChartData,
  layout: 'vertical' | 'horizontal',
  canDrilldown?: boolean,
  onDrilldown?: (label: string) => void
) {
  const isHorizontal = layout === 'horizontal';

  return (
    <BarChart
      data={chartData}
      layout={isHorizontal ? 'vertical' : 'horizontal'}
      margin={{ top: 5, right: 30, left: isHorizontal ? 80 : 20, bottom: 5 }}
    >
      <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
      {isHorizontal ? (
        <>
          <XAxis type="number" tick={{ fontSize: 11, fill: '#78716c' }} />
          <YAxis
            type="category"
            dataKey="name"
            tick={{ fontSize: 11, fill: '#78716c' }}
            width={70}
          />
        </>
      ) : (
        <>
          <XAxis
            type="category"
            dataKey="name"
            tick={{ fontSize: 11, fill: '#78716c' }}
            angle={-45}
            textAnchor="end"
            height={60}
          />
          <YAxis type="number" tick={{ fontSize: 11, fill: '#78716c' }} />
        </>
      )}
      <Tooltip
        formatter={(value) => {
          const numValue = typeof value === 'number' ? value : 0;
          return [`${numValue}`, data.y_label || 'Count'];
        }}
        contentStyle={{
          backgroundColor: 'white',
          border: '1px solid #e7e5e4',
          borderRadius: '8px',
          fontSize: '12px',
        }}
      />
      <Bar
        dataKey="value"
        fill={CHART_COLORS[0]}
        radius={[4, 4, 0, 0]}
        onClick={canDrilldown ? (entry) => {
          if (entry?.name) onDrilldown?.(String(entry.name));
        } : undefined}
        style={canDrilldown ? { cursor: 'pointer' } : undefined}
      >
        {chartData.map((_, index) => (
          <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
        ))}
      </Bar>
    </BarChart>
  );
}

function renderLineChart(chartData: ChartDataItem[], data: ChartData) {
  return (
    <LineChart data={chartData} margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
      <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
      <XAxis dataKey="name" tick={{ fontSize: 11, fill: '#78716c' }} />
      <YAxis tick={{ fontSize: 11, fill: '#78716c' }} />
      <Tooltip
        formatter={(value) => {
          const numValue = typeof value === 'number' ? value : 0;
          return [numValue, data.y_label || 'Count'];
        }}
        contentStyle={{
          backgroundColor: 'white',
          border: '1px solid #e7e5e4',
          borderRadius: '8px',
          fontSize: '12px',
        }}
      />
      <Line
        type="monotone"
        dataKey="value"
        stroke={CHART_COLORS[0]}
        strokeWidth={2}
        dot={{ fill: CHART_COLORS[0], strokeWidth: 2 }}
        activeDot={{ r: 6 }}
      />
    </LineChart>
  );
}

export default AnalyticsChart;
