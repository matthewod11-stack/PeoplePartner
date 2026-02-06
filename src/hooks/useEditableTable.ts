/**
 * useEditableTable Hook
 *
 * Manages inline cell editing state for the fix-and-retry step.
 * Tracks which cells have been modified and provides edit/commit/cancel actions.
 */

import { useState, useCallback } from 'react';
import type { ParsedRow } from '../lib/types';

// =============================================================================
// Types
// =============================================================================

export interface EditableTableState {
  rows: ParsedRow[];
  editingCell: { rowIndex: number; columnKey: string } | null;
  modifiedCells: Set<string>;
  hasChanges: boolean;
}

export interface EditableTableActions {
  startEdit: (rowIndex: number, columnKey: string) => void;
  commitEdit: (value: string) => void;
  cancelEdit: () => void;
  getModifiedRows: () => ParsedRow[];
  resetChanges: () => void;
}

interface UseEditableTableOptions {
  initialRows: ParsedRow[];
}

// =============================================================================
// Hook
// =============================================================================

function cellKey(rowIndex: number, columnKey: string): string {
  return `${rowIndex}:${columnKey}`;
}

export function useEditableTable(
  options: UseEditableTableOptions
): [EditableTableState, EditableTableActions] {
  const [rows, setRows] = useState<ParsedRow[]>(
    () => options.initialRows.map((r) => ({ ...r }))
  );
  const [editingCell, setEditingCell] = useState<{
    rowIndex: number;
    columnKey: string;
  } | null>(null);
  const [modifiedCells, setModifiedCells] = useState<Set<string>>(new Set());

  const startEdit = useCallback((rowIndex: number, columnKey: string) => {
    setEditingCell({ rowIndex, columnKey });
  }, []);

  const commitEdit = useCallback(
    (value: string) => {
      if (!editingCell) return;

      const { rowIndex, columnKey } = editingCell;

      setRows((prev) => {
        const updated = [...prev];
        updated[rowIndex] = { ...updated[rowIndex], [columnKey]: value };
        return updated;
      });

      setModifiedCells((prev) => {
        const next = new Set(prev);
        next.add(cellKey(rowIndex, columnKey));
        return next;
      });

      setEditingCell(null);
    },
    [editingCell]
  );

  const cancelEdit = useCallback(() => {
    setEditingCell(null);
  }, []);

  const getModifiedRows = useCallback(() => {
    return rows;
  }, [rows]);

  const resetChanges = useCallback(() => {
    setRows(options.initialRows.map((r) => ({ ...r })));
    setModifiedCells(new Set());
    setEditingCell(null);
  }, [options.initialRows]);

  const state: EditableTableState = {
    rows,
    editingCell,
    modifiedCells,
    hasChanges: modifiedCells.size > 0,
  };

  const actions: EditableTableActions = {
    startEdit,
    commitEdit,
    cancelEdit,
    getModifiedRows,
    resetChanges,
  };

  return [state, actions];
}
