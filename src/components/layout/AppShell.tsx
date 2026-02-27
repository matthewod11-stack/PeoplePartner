import { type ReactNode } from 'react';
import { useLayout } from '../../contexts/LayoutContext';
import { useNetwork } from '../../hooks';
import { useUpdateCheck } from '../../hooks/useUpdateCheck';
import { OfflineIndicator } from '../shared';
import { TabSwitcher, ConversationSidebar } from '../conversations';
import { EmployeePanel } from '../employees';
import { TrialBanner } from '../trial/TrialBanner';

interface AppShellProps {
  children: ReactNode;
  contextPanel?: ReactNode;
  /** Handler for settings button click */
  onSettingsClick?: () => void;
}

function ToggleButton({
  onClick,
  direction,
  label
}: {
  onClick: () => void;
  direction: 'left' | 'right';
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      className="
        w-10 h-10 flex items-center justify-center
        text-stone-500 hover:text-stone-700
        hover:bg-stone-200/50 rounded-md
        transition-colors duration-200
      "
    >
      <svg
        className={`w-4 h-4 transition-transform duration-300 ${direction === 'left' ? '' : 'rotate-180'}`}
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" />
      </svg>
    </button>
  );
}

function IconButton({
  onClick,
  label,
  shortcut,
  children
}: {
  onClick?: () => void;
  label: string;
  shortcut?: string;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      title={shortcut ? `${label} (${shortcut})` : label}
      className="
        w-8 h-8 flex items-center justify-center
        text-stone-500 hover:text-stone-700
        hover:bg-stone-200/60 rounded-md
        transition-all duration-200
        hover:brightness-110 active:brightness-90
      "
    >
      {children}
    </button>
  );
}

export function AppShell({ children, contextPanel, onSettingsClick }: AppShellProps) {
  const { sidebarOpen, contextPanelOpen, sidebarTab, toggleSidebar, toggleContextPanel, setSidebarTab } = useLayout();
  const { isOnline, errorMessage, checkNow, isChecking } = useNetwork();
  const { updateAvailable, installing, installUpdate } = useUpdateCheck();

  return (
    <div className="h-screen flex flex-col bg-stone-50 overflow-hidden">
      {/* Header */}
      <header className="
        h-12 flex-shrink-0
        flex items-center justify-between
        px-4
        bg-gradient-to-b from-stone-100 to-stone-100/95
        border-b border-stone-200/60
        shadow-sm
      ">
        <div className="flex items-center gap-3">
          {/* Sidebar toggle */}
          <ToggleButton
            onClick={toggleSidebar}
            direction={sidebarOpen ? 'left' : 'right'}
            label={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
          />

          {/* App title */}
          <h1 className="font-display text-lg font-semibold text-stone-800 tracking-tight">
            HR Command Center
          </h1>
        </div>

        {/* Network status indicator */}
        <OfflineIndicator
          isOffline={!isOnline}
          errorMessage={errorMessage}
          onRetry={checkNow}
          isChecking={isChecking}
        />

        <div className="flex items-center gap-1">
          {updateAvailable && (
            <button
              type="button"
              onClick={() => { void installUpdate(); }}
              disabled={installing}
              className="
                px-2 py-1 mr-1
                text-xs font-medium
                text-blue-700
                bg-blue-100 hover:bg-blue-200
                rounded-md
                transition-colors duration-150
                disabled:opacity-70 disabled:cursor-wait
              "
            >
              {installing ? 'Installing Update...' : 'Update Available'}
            </button>
          )}

          {/* Command palette hint */}
          <button
            onClick={() => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))}
            className="
              hidden sm:flex items-center gap-1.5
              px-2 py-1 mr-1
              text-xs text-stone-500 hover:text-stone-700
              bg-stone-100 hover:bg-stone-200/80
              border border-stone-200/60
              rounded-md
              transition-colors duration-150
            "
            title="Open command palette"
          >
            <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <kbd className="font-mono">⌘K</kbd>
          </button>

          {/* Help button */}
          <IconButton label="Help" shortcut="⌘K">
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9 5.25h.008v.008H12v-.008z" />
            </svg>
          </IconButton>

          {/* Settings button */}
          <IconButton label="Settings" shortcut="⌘," onClick={onSettingsClick}>
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </IconButton>

          {/* Context panel toggle */}
          <div className="ml-2 pl-2 border-l border-stone-200/60">
            <ToggleButton
              onClick={toggleContextPanel}
              direction={contextPanelOpen ? 'right' : 'left'}
              label={contextPanelOpen ? 'Collapse context panel' : 'Expand context panel'}
            />
          </div>
        </div>
      </header>

      {/* Trial banner */}
      <TrialBanner />

      {/* Main content area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <aside
          className={`
            flex-shrink-0
            bg-gradient-to-br from-stone-100 to-stone-100/90
            border-r border-stone-200/60
            transition-all duration-300 ease-in-out
            overflow-hidden
            ${sidebarOpen ? 'w-72' : 'w-0'}
          `}
        >
          <div className={`
            w-72 h-full flex flex-col
            transition-opacity duration-200
            ${sidebarOpen ? 'opacity-100' : 'opacity-0'}
          `}>
            {/* Tab Switcher */}
            <div className="p-3 pb-0">
              <TabSwitcher value={sidebarTab} onChange={setSidebarTab} />
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-hidden">
              {sidebarTab === 'conversations' ? (
                <ConversationSidebar />
              ) : (
                <EmployeePanel />
              )}
            </div>
          </div>
        </aside>

        {/* Main chat area */}
        <main className="
          flex-1
          flex flex-col
          overflow-hidden
          bg-stone-50
        ">
          <div className="flex-1 overflow-y-auto">
            <div className="max-w-[720px] mx-auto h-full px-6 py-4">
              {children}
            </div>
          </div>
        </main>

        {/* Context panel */}
        <aside
          className={`
            flex-shrink-0
            bg-gradient-to-bl from-stone-100 to-stone-100/90
            border-l border-stone-200/60
            transition-all duration-300 ease-in-out
            overflow-hidden
            ${contextPanelOpen ? 'w-[280px]' : 'w-0'}
          `}
        >
          <div className={`
            w-[280px] h-full
            transition-opacity duration-200
            ${contextPanelOpen ? 'opacity-100' : 'opacity-0'}
          `}>
            {contextPanel || <ContextPanelPlaceholder />}
          </div>
        </aside>
      </div>
    </div>
  );
}

function ContextPanelPlaceholder() {
  return (
    <div className="h-full flex flex-col p-4">
      <p className="text-xs font-medium text-stone-500 uppercase tracking-wider mb-3">
        Employee Context
      </p>

      <div className="
        p-4 rounded-lg
        bg-white/60
        border border-stone-200/40
        shadow-sm
      ">
        <div className="flex items-center gap-3 mb-3">
          <div className="
            w-10 h-10 rounded-full
            bg-primary-100
            flex items-center justify-center
            text-primary-600 font-medium
          ">
            SC
          </div>
          <div>
            <p className="font-medium text-stone-800">Sarah Chen</p>
            <p className="text-sm text-stone-500">Marketing Manager</p>
          </div>
        </div>

        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-stone-500">Location</span>
            <span className="text-stone-700">California</span>
          </div>
          <div className="flex justify-between">
            <span className="text-stone-500">Hired</span>
            <span className="text-stone-700">Mar 2021</span>
          </div>
          <div className="flex justify-between">
            <span className="text-stone-500">Status</span>
            <span className="text-primary-600 font-medium">Active</span>
          </div>
        </div>
      </div>

      <div className="mt-auto pt-4">
        <div className="
          p-4 rounded-lg
          border-2 border-dashed border-stone-200
          text-center
          hover:border-primary-300 hover:bg-primary-50/30
          transition-colors duration-200
          cursor-pointer
        ">
          <svg className="w-6 h-6 mx-auto text-stone-500 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
          </svg>
          <p className="text-sm text-stone-500">Import CSV</p>
        </div>
      </div>
    </div>
  );
}

export default AppShell;
