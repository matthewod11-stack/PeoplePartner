/**
 * PrintableReport Component (V2.3.2k)
 *
 * Hidden container that renders a print-friendly version of an insight board.
 * Visible only during print via CSS @media print rules.
 */

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
  CartesianGrid,
} from 'recharts';
import type { ChartData, ChartType } from '../../lib/analytics-types';
import type { InsightBoard, PinnedChart, ChartAnnotation } from '../../lib/tauri-commands';
import { parseChartData } from '../../lib/insight-canvas-types';

interface PrintableReportProps {
  /** The board being printed */
  board: InsightBoard;
  /** Pinned charts on the board */
  charts: PinnedChart[];
  /** Annotations per chart */
  annotations: Record<string, ChartAnnotation[]>;
}

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

const TYPE_STYLES: Record<string, { bg: string; text: string }> = {
  note: { bg: '#f5f5f4', text: '#57534e' }, // stone-100, stone-600
  callout: { bg: '#fef3c7', text: '#b45309' }, // amber-100, amber-700
  question: { bg: '#ede9fe', text: '#7c3aed' }, // primary-100, primary-700
};

export function PrintableReport({ board, charts, annotations }: PrintableReportProps) {
  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    });
  };

  const now = new Date().toLocaleString('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });

  return (
    <div className="print-container" style={{ fontFamily: 'system-ui, sans-serif' }}>
      {/* Header */}
      <div style={{ marginBottom: '32px', borderBottom: '2px solid #e7e5e4', paddingBottom: '16px' }}>
        <h1 style={{ fontSize: '24px', fontWeight: 'bold', color: '#1c1917', margin: 0 }}>
          {board.name}
        </h1>
        <p style={{ fontSize: '14px', color: '#78716c', marginTop: '8px' }}>
          {charts.length} {charts.length === 1 ? 'chart' : 'charts'} · Last updated {formatDate(board.updated_at)}
        </p>
      </div>

      {/* Charts */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '24px' }}>
        {charts.map((pinnedChart) => {
          const chartData = parseChartData(pinnedChart);
          const chartAnnotations = annotations[pinnedChart.id] || [];

          if (!chartData) return null;

          const data = chartData.data.map((point) => ({
            name: point.label,
            value: point.value,
            percentage: point.percentage ?? 0,
          }));

          return (
            <div key={pinnedChart.id} className="print-chart" style={{ background: '#fff' }}>
              {/* Chart title */}
              <h2 style={{ fontSize: '14px', fontWeight: '600', color: '#1c1917', marginBottom: '12px' }}>
                {chartData.title}
              </h2>

              {/* Chart */}
              <div style={{ width: '100%', height: '200px' }}>
                {renderPrintChart(chartData.chart_type, data, chartData)}
              </div>

              {/* Filters applied */}
              {chartData.filters_applied && (
                <p style={{ fontSize: '11px', color: '#a8a29e', marginTop: '8px' }}>
                  {chartData.filters_applied}
                </p>
              )}

              {/* Annotations */}
              {chartAnnotations.length > 0 && (
                <div style={{ marginTop: '12px', paddingTop: '12px', borderTop: '1px solid #e7e5e4' }}>
                  {chartAnnotations.map((annotation) => {
                    const style = TYPE_STYLES[annotation.annotation_type] || TYPE_STYLES.note;
                    return (
                      <div
                        key={annotation.id}
                        style={{
                          display: 'flex',
                          gap: '8px',
                          marginBottom: '8px',
                          padding: '8px',
                          background: '#fafaf9',
                          borderRadius: '4px',
                        }}
                      >
                        <span
                          style={{
                            padding: '2px 8px',
                            borderRadius: '4px',
                            fontSize: '10px',
                            fontWeight: '500',
                            background: style.bg,
                            color: style.text,
                          }}
                        >
                          {annotation.annotation_type.charAt(0).toUpperCase() + annotation.annotation_type.slice(1)}
                        </span>
                        <span style={{ fontSize: '12px', color: '#44403c' }}>
                          {annotation.content}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Footer */}
      <div style={{ marginTop: '48px', paddingTop: '16px', borderTop: '1px solid #e7e5e4' }}>
        <p style={{ fontSize: '11px', color: '#a8a29e', textAlign: 'center' }}>
          Generated from HR Command Center · {now}
        </p>
      </div>
    </div>
  );
}

interface ChartDataItem {
  name: string;
  value: number;
  percentage: number;
  [key: string]: string | number; // Index signature for Recharts compatibility
}

function renderPrintChart(chartType: ChartType, data: ChartDataItem[], _chartData: ChartData) {
  switch (chartType) {
    case 'pie':
      return (
        <PieChart width={280} height={200}>
          <Pie
            data={data}
            dataKey="value"
            nameKey="name"
            cx="50%"
            cy="50%"
            outerRadius={60}
            label={({ name, payload }) => `${name} (${payload?.percentage ?? 0}%)`}
            labelLine={{ stroke: '#78716c', strokeWidth: 1 }}
          >
            {data.map((_, index) => (
              <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
            ))}
          </Pie>
          <Tooltip />
          <Legend wrapperStyle={{ fontSize: '10px' }} />
        </PieChart>
      );

    case 'bar':
      return (
        <BarChart width={280} height={200} data={data} margin={{ top: 5, right: 20, left: 10, bottom: 40 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
          <XAxis dataKey="name" tick={{ fontSize: 9, fill: '#78716c' }} angle={-45} textAnchor="end" height={50} />
          <YAxis tick={{ fontSize: 9, fill: '#78716c' }} />
          <Tooltip />
          <Bar dataKey="value" radius={[4, 4, 0, 0]}>
            {data.map((_, index) => (
              <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
            ))}
          </Bar>
        </BarChart>
      );

    case 'horizontal_bar':
      return (
        <BarChart width={280} height={200} data={data} layout="vertical" margin={{ top: 5, right: 20, left: 60, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
          <XAxis type="number" tick={{ fontSize: 9, fill: '#78716c' }} />
          <YAxis type="category" dataKey="name" tick={{ fontSize: 9, fill: '#78716c' }} width={55} />
          <Tooltip />
          <Bar dataKey="value" radius={[0, 4, 4, 0]}>
            {data.map((_, index) => (
              <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
            ))}
          </Bar>
        </BarChart>
      );

    case 'line':
      return (
        <LineChart width={280} height={200} data={data} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
          <XAxis dataKey="name" tick={{ fontSize: 9, fill: '#78716c' }} />
          <YAxis tick={{ fontSize: 9, fill: '#78716c' }} />
          <Tooltip />
          <Line type="monotone" dataKey="value" stroke={CHART_COLORS[0]} strokeWidth={2} dot={{ fill: CHART_COLORS[0] }} />
        </LineChart>
      );

    default:
      return (
        <BarChart width={280} height={200} data={data} margin={{ top: 5, right: 20, left: 10, bottom: 40 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#e7e5e4" />
          <XAxis dataKey="name" tick={{ fontSize: 9, fill: '#78716c' }} angle={-45} textAnchor="end" height={50} />
          <YAxis tick={{ fontSize: 9, fill: '#78716c' }} />
          <Tooltip />
          <Bar dataKey="value" radius={[4, 4, 0, 0]}>
            {data.map((_, index) => (
              <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
            ))}
          </Bar>
        </BarChart>
      );
  }
}

export default PrintableReport;
