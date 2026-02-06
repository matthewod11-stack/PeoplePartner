/**
 * CommandPalette Component
 *
 * A VS Code/Slack-style command palette for quick navigation.
 * Supports fuzzy search across actions, conversations, and employees.
 */

import { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import Fuse, { type IFuseOptions } from 'fuse.js';
import {
  useConversationActions,
  useConversationDirectory,
} from '../contexts/ConversationContext';
import { useEmployees } from '../contexts/EmployeeContext';
import { useLayout } from '../contexts/LayoutContext';

// =============================================================================
// Types
// =============================================================================

type CommandType = 'action' | 'conversation' | 'employee';

interface CommandItem {
  id: string;
  type: CommandType;
  title: string;
  subtitle?: string;
  shortcut?: string;
  onSelect: () => void;
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
}

// =============================================================================
// Icons
// =============================================================================

function SearchIcon() {
  return (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
      <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>
  );
}

function ActionIcon() {
  return (
    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
      <path strokeLinecap="round" strokeLinejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
    </svg>
  );
}

function ConversationIcon() {
  return (
    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
      <path strokeLinecap="round" strokeLinejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
    </svg>
  );
}

function EmployeeIcon() {
  return (
    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
      <path strokeLinecap="round" strokeLinejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
    </svg>
  );
}

// =============================================================================
// Static Actions
// =============================================================================

const STATIC_ACTIONS: Omit<CommandItem, 'onSelect'>[] = [
  { id: 'new-conversation', type: 'action', title: 'New Conversation', shortcut: '⌘N' },
  { id: 'open-settings', type: 'action', title: 'Open Settings', shortcut: '⌘,' },
  { id: 'focus-chat', type: 'action', title: 'Focus Chat Input', shortcut: '⌘/' },
  { id: 'switch-employees', type: 'action', title: 'Show Employees', shortcut: '⌘E' },
  { id: 'switch-conversations', type: 'action', title: 'Show Conversations' },
  { id: 'import-employees', type: 'action', title: 'Import Employees' },
  { id: 'toggle-sidebar', type: 'action', title: 'Toggle Sidebar' },
  { id: 'toggle-context', type: 'action', title: 'Toggle Context Panel' },
];

// =============================================================================
// Fuse.js Configuration
// =============================================================================

const fuseOptions: IFuseOptions<CommandItem> = {
  keys: [
    { name: 'title', weight: 0.7 },
    { name: 'subtitle', weight: 0.3 },
  ],
  threshold: 0.4,
  distance: 100,
  includeScore: true,
};

// =============================================================================
// Result Item Component
// =============================================================================

function ResultItem({
  item,
  isSelected,
  optionId,
  onSelect,
  onMouseEnter,
}: {
  item: CommandItem;
  isSelected: boolean;
  optionId: string;
  onSelect: () => void;
  onMouseEnter: () => void;
}) {
  const getIcon = () => {
    switch (item.type) {
      case 'action':
        return <ActionIcon />;
      case 'conversation':
        return <ConversationIcon />;
      case 'employee':
        return <EmployeeIcon />;
    }
  };

  return (
    <button
      onClick={onSelect}
      onMouseEnter={onMouseEnter}
      id={optionId}
      role="option"
      aria-selected={isSelected}
      className={`
        w-full flex items-center gap-3 px-4 py-2.5
        text-left transition-colors duration-75
        ${isSelected
          ? 'bg-primary-50 text-primary-700'
          : 'text-stone-700 hover:bg-stone-50'
        }
      `}
    >
      {/* Icon */}
      <div
        className={`
          w-8 h-8 flex items-center justify-center rounded-lg
          ${isSelected ? 'bg-primary-100 text-primary-600' : 'bg-stone-100 text-stone-500'}
        `}
      >
        {getIcon()}
      </div>

      {/* Title + Subtitle */}
      <div className="flex-1 min-w-0">
        <p className="font-medium truncate">{item.title}</p>
        {item.subtitle && (
          <p className="text-sm text-stone-500 truncate">{item.subtitle}</p>
        )}
      </div>

      {/* Shortcut hint */}
      {item.shortcut && (
        <kbd
          className={`
            px-1.5 py-0.5 text-xs font-mono rounded
            ${isSelected
              ? 'bg-primary-100 text-primary-600 border border-primary-200'
              : 'bg-stone-100 text-stone-500 border border-stone-200'
            }
          `}
        >
          {item.shortcut}
        </kbd>
      )}
    </button>
  );
}

// =============================================================================
// Result Group Component
// =============================================================================

function ResultGroup({
  title,
  items,
  selectedIndex,
  startIndex,
  onSelect,
  onItemHover,
  getOptionId,
}: {
  title: string;
  items: CommandItem[];
  selectedIndex: number;
  startIndex: number;
  onSelect: (item: CommandItem) => void;
  onItemHover: (index: number) => void;
  getOptionId: (item: CommandItem) => string;
}) {
  if (items.length === 0) return null;

  return (
    <div className="py-2">
      <p className="px-4 pb-1.5 text-xs font-medium text-stone-500 uppercase tracking-wide">
        {title}
      </p>
      {items.map((item, i) => (
        <ResultItem
          key={item.id}
          item={item}
          isSelected={selectedIndex === startIndex + i}
          optionId={getOptionId(item)}
          onSelect={() => onSelect(item)}
          onMouseEnter={() => onItemHover(startIndex + i)}
        />
      ))}
    </div>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function CommandPalette({ isOpen, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Contexts
  const { conversations } = useConversationDirectory();
  const { loadConversation, startNewConversation } = useConversationActions();
  const { employees, selectEmployee, openImportWizard } = useEmployees();
  const { setSidebarTab, setSidebarOpen, toggleSidebar, toggleContextPanel, setContextPanelOpen } = useLayout();

  // Build action handlers
  const actionHandlers = useMemo(
    () => ({
      'new-conversation': () => {
        selectEmployee(null); // Clear employee selection for fresh start
        startNewConversation();
        onClose();
      },
      'open-settings': () => {
        // Settings is handled by the parent via keyboard shortcut
        // Dispatch a synthetic Cmd+, event
        window.dispatchEvent(new KeyboardEvent('keydown', { key: ',', metaKey: true }));
        onClose();
      },
      'focus-chat': () => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: '/', metaKey: true }));
        onClose();
      },
      'switch-employees': () => {
        setSidebarTab('employees');
        setSidebarOpen(true);
        onClose();
      },
      'switch-conversations': () => {
        setSidebarTab('conversations');
        setSidebarOpen(true);
        onClose();
      },
      'import-employees': () => {
        openImportWizard();
        onClose();
      },
      'toggle-sidebar': () => {
        toggleSidebar();
        onClose();
      },
      'toggle-context': () => {
        toggleContextPanel();
        onClose();
      },
    }),
    [selectEmployee, startNewConversation, setSidebarTab, setSidebarOpen, openImportWizard, toggleSidebar, toggleContextPanel, onClose]
  );

  // Build all searchable items
  const allItems = useMemo((): CommandItem[] => {
    // Actions
    const actionItems: CommandItem[] = STATIC_ACTIONS.map((a) => ({
      ...a,
      onSelect: actionHandlers[a.id as keyof typeof actionHandlers],
    }));

    // Conversations (most recent 20)
    const conversationItems: CommandItem[] = conversations.slice(0, 20).map((c) => ({
      id: `conv-${c.id}`,
      type: 'conversation' as const,
      title: c.title || 'Untitled Conversation',
      subtitle: c.first_message_preview
        ? c.first_message_preview.slice(0, 60) + (c.first_message_preview.length > 60 ? '...' : '')
        : undefined,
      onSelect: () => {
        loadConversation(c.id);
        onClose();
      },
    }));

    // Employees (first 50)
    const employeeItems: CommandItem[] = employees.slice(0, 50).map((e) => ({
      id: `emp-${e.id}`,
      type: 'employee' as const,
      title: e.full_name,
      subtitle: [e.job_title, e.department].filter(Boolean).join(' · '),
      onSelect: () => {
        selectEmployee(e.id);
        setContextPanelOpen(true);
        onClose();
      },
    }));

    return [...actionItems, ...conversationItems, ...employeeItems];
  }, [conversations, employees, actionHandlers, loadConversation, selectEmployee, setContextPanelOpen, onClose]);

  const fuse = useMemo(() => new Fuse(allItems, fuseOptions), [allItems]);

  // Filter/search items
  const filteredItems = useMemo(() => {
    if (!query.trim()) {
      // No query: show prioritized items
      return {
        actions: allItems.filter((i) => i.type === 'action').slice(0, 5),
        conversations: allItems.filter((i) => i.type === 'conversation').slice(0, 5),
        employees: allItems.filter((i) => i.type === 'employee').slice(0, 5),
      };
    }

    // With query: use Fuse.js
    const results = fuse.search(query).map((r) => r.item);

    return {
      actions: results.filter((i) => i.type === 'action').slice(0, 5),
      conversations: results.filter((i) => i.type === 'conversation').slice(0, 5),
      employees: results.filter((i) => i.type === 'employee').slice(0, 8),
    };
  }, [query, allItems, fuse]);

  // Flat list for keyboard navigation
  const flatResults = useMemo(
    () => [...filteredItems.actions, ...filteredItems.conversations, ...filteredItems.employees],
    [filteredItems]
  );

  const getOptionId = useCallback((item: CommandItem) => `command-option-${item.id}`, []);

  // Reset selected index when results change
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Focus input when opened
  useEffect(() => {
    if (isOpen) {
      setQuery('');
      setSelectedIndex(0);
      // Small delay to ensure portal is rendered
      setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [isOpen]);

  // Handle keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex((prev) => Math.min(prev + 1, flatResults.length - 1));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex((prev) => Math.max(prev - 1, 0));
          break;
        case 'Enter':
          e.preventDefault();
          if (flatResults[selectedIndex]) {
            flatResults[selectedIndex].onSelect();
          }
          break;
        case 'Escape':
          e.preventDefault();
          onClose();
          break;
      }
    },
    [flatResults, selectedIndex, onClose]
  );

  // Handle item selection
  const handleSelect = useCallback((item: CommandItem) => {
    item.onSelect();
  }, []);

  // Handle mouse hover
  const handleItemHover = useCallback((index: number) => {
    setSelectedIndex(index);
  }, []);

  if (!isOpen) return null;

  // Calculate start indices for each group
  const actionStartIndex = 0;
  const conversationStartIndex = filteredItems.actions.length;
  const employeeStartIndex = filteredItems.actions.length + filteredItems.conversations.length;

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-stone-900/50 backdrop-blur-sm animate-fade-in"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Palette */}
      <div
        className="
          relative w-full max-w-xl
          bg-white rounded-xl shadow-2xl
          animate-fade-in
          overflow-hidden
        "
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-stone-200">
          <div className="text-stone-500">
            <SearchIcon />
          </div>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search commands, conversations, employees..."
            className="
              flex-1 text-base text-stone-700
              placeholder:text-stone-400
              bg-transparent border-none outline-none
            "
            role="combobox"
            aria-expanded={flatResults.length > 0}
            aria-controls="command-results"
            aria-activedescendant={
              flatResults[selectedIndex] ? getOptionId(flatResults[selectedIndex]) : undefined
            }
          />
          <kbd className="px-1.5 py-0.5 text-xs font-mono bg-stone-100 text-stone-500 border border-stone-200 rounded">
            esc
          </kbd>
        </div>

        {/* Results */}
        <div
          id="command-results"
          role="listbox"
          className="max-h-[60vh] overflow-y-auto"
        >
          {flatResults.length === 0 ? (
            <div className="px-4 py-8 text-center text-stone-500">
              No results found for "{query}"
            </div>
          ) : (
            <>
              <ResultGroup
                title="Actions"
                items={filteredItems.actions}
                selectedIndex={selectedIndex}
                startIndex={actionStartIndex}
                onSelect={handleSelect}
                onItemHover={handleItemHover}
                getOptionId={getOptionId}
              />
              <ResultGroup
                title="Conversations"
                items={filteredItems.conversations}
                selectedIndex={selectedIndex}
                startIndex={conversationStartIndex}
                onSelect={handleSelect}
                onItemHover={handleItemHover}
                getOptionId={getOptionId}
              />
              <ResultGroup
                title="Employees"
                items={filteredItems.employees}
                selectedIndex={selectedIndex}
                startIndex={employeeStartIndex}
                onSelect={handleSelect}
                onItemHover={handleItemHover}
                getOptionId={getOptionId}
              />
            </>
          )}
        </div>

        {/* Footer hint */}
        <div className="px-4 py-2 border-t border-stone-100 bg-stone-50">
          <p className="text-xs text-stone-500">
            <kbd className="px-1 py-0.5 font-mono bg-white border border-stone-200 rounded text-stone-500">↑↓</kbd>
            {' '}to navigate
            <span className="mx-2">·</span>
            <kbd className="px-1 py-0.5 font-mono bg-white border border-stone-200 rounded text-stone-500">↵</kbd>
            {' '}to select
            <span className="mx-2">·</span>
            <kbd className="px-1 py-0.5 font-mono bg-white border border-stone-200 rounded text-stone-500">esc</kbd>
            {' '}to close
          </p>
        </div>
      </div>
    </div>,
    document.body
  );
}

export default CommandPalette;
