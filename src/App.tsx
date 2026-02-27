import { useState, useCallback, useEffect, useRef, lazy, Suspense, Component, type ReactNode } from 'react';
import { LayoutProvider } from './contexts/LayoutContext';
import { EmployeeProvider } from './contexts/EmployeeContext';
import {
  ConversationProvider,
  useConversationMessages,
  useConversationMeta,
  useConversationActions,
} from './contexts/ConversationContext';
import { TrialProvider, useTrial } from './contexts/TrialContext';
import { AppShell } from './components/layout/AppShell';
import { ChatInput, MessageList, type ChatInputHandle } from './components/chat';
import { PIINotification } from './components/shared';
import { Badge } from './components/ui/Badge';
import { EmployeeDetail } from './components/employees';
import { TestDataImporter } from './components/dev/TestDataImporter';
import { OnboardingProvider, OnboardingFlow, useOnboarding } from './components/onboarding';
import { useEmployees } from './contexts/EmployeeContext';
import { useNetwork, useCommandPalette } from './hooks';

const EmployeeEdit = lazy(() =>
  import('./components/employees/EmployeeEdit').then((module) => ({ default: module.EmployeeEdit }))
);
const ImportWizard = lazy(() =>
  import('./components/import/ImportWizard').then((module) => ({ default: module.ImportWizard }))
);
const SettingsPanel = lazy(() =>
  import('./components/settings/SettingsPanel').then((module) => ({ default: module.SettingsPanel }))
);
const CommandPalette = lazy(() => import('./components/CommandPalette'));
const UpgradePrompt = lazy(() =>
  import('./components/trial/UpgradePrompt').then((module) => ({ default: module.UpgradePrompt }))
);

// Error Boundary to catch React render errors
class ErrorBoundary extends Component<{ children: ReactNode }, { hasError: boolean; error: Error | null }> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('[ErrorBoundary] Caught error:', error);
    console.error('[ErrorBoundary] Component stack:', errorInfo.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-red-50 p-8">
          <div className="max-w-xl bg-white rounded-xl shadow-lg p-6">
            <h1 className="text-xl font-bold text-red-600 mb-4">Something went wrong</h1>
            <pre className="bg-red-100 p-4 rounded text-sm overflow-auto text-red-800">
              {this.state.error?.message}
            </pre>
            <button
              onClick={() => window.location.reload()}
              className="mt-4 px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
            >
              Reload App
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

interface ChatAreaProps {
  chatInputRef?: React.RefObject<ChatInputHandle>;
}

function ChatArea({ chatInputRef }: ChatAreaProps) {
  // Get conversation state from context
  const { messages, isLoading } = useConversationMessages();
  const { piiNotification } = useConversationMeta();
  const {
    sendMessage,
    retryMessage,
    startNewConversation,
    clearPiiNotification,
  } = useConversationActions();

  // Get selected employee from context (for prioritizing in context builder)
  const { selectedEmployeeId, selectEmployee } = useEmployees();

  // Trial status for message counter badge
  const { isTrialMode, trialStatus, messagesRemaining, isAtMessageLimit, refreshTrialStatus } = useTrial();

  // Get network state for offline mode
  const { isOnline, isApiReachable } = useNetwork();
  const isOffline = !isOnline || !isApiReachable;

  // Keyboard shortcut: Cmd+N to start a new conversation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === 'n' && !e.shiftKey) {
        e.preventDefault();
        selectEmployee(null); // Clear employee selection for fresh start
        startNewConversation();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectEmployee, startNewConversation]);

  const handleSubmit = useCallback(
    async (content: string) => {
      try {
        // Pass selected employee ID to prioritize in context builder
        await sendMessage(content, selectedEmployeeId);
      } finally {
        // Refresh trial counters even when a request fails (for server-authoritative limits).
        await refreshTrialStatus();
      }
    },
    [sendMessage, selectedEmployeeId, refreshTrialStatus]
  );

  const handlePromptClick = useCallback(
    (prompt: string) => {
      handleSubmit(prompt);
    },
    [handleSubmit]
  );

  // Copy message to clipboard (for failed message recovery)
  const handleCopyMessage = useCallback((content: string) => {
    navigator.clipboard.writeText(content).catch((err) => {
      console.error('[ChatArea] Failed to copy to clipboard:', err);
    });
  }, []);

  return (
    <div className="h-full flex flex-col">
      {/* Trial message counter badge */}
      {isTrialMode && trialStatus && (
        <div className="flex justify-end px-2 pt-1">
          <Badge
            variant={messagesRemaining <= 5 ? 'warning' : 'default'}
            size="sm"
          >
            {trialStatus.messages_used}/{trialStatus.messages_limit} messages
          </Badge>
        </div>
      )}
      {/* PII redaction notification */}
      <PIINotification
        summary={piiNotification}
        onDismiss={clearPiiNotification}
      />
      <MessageList
        messages={messages}
        isLoading={isLoading}
        onPromptClick={handlePromptClick}
        onRetry={retryMessage}
        onCopyMessage={handleCopyMessage}
      />
      <ChatInput
        ref={chatInputRef}
        onSubmit={handleSubmit}
        disabled={isLoading || isAtMessageLimit}
        isOffline={isOffline}
      />
    </div>
  );
}

function EmployeeEditModal() {
  const {
    selectedEmployee,
    isEditModalOpen,
    closeEditModal,
    updateEmployeeInList,
  } = useEmployees();

  if (!selectedEmployee) return null;

  return (
    <Suspense fallback={null}>
      <EmployeeEdit
        employee={selectedEmployee}
        isOpen={isEditModalOpen}
        onClose={closeEditModal}
        onSave={updateEmployeeInList}
      />
    </Suspense>
  );
}

function ImportWizardModal() {
  const { isImportWizardOpen, closeImportWizard, refreshEmployees } = useEmployees();

  return (
    <Suspense fallback={null}>
      <ImportWizard
        isOpen={isImportWizardOpen}
        onClose={closeImportWizard}
        onComplete={refreshEmployees}
      />
    </Suspense>
  );
}

// Developer modal for test data import (Cmd+Shift+T)
function TestDataModal({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const { refreshEmployees } = useEmployees();

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/50"
        onClick={() => {
          onClose();
          refreshEmployees();
        }}
      />
      <div className="relative bg-white rounded-xl shadow-2xl max-w-2xl w-full max-h-[80vh] overflow-auto">
        <button
          onClick={() => {
            onClose();
            refreshEmployees();
          }}
          className="absolute top-4 right-4 text-gray-400 hover:text-gray-600"
        >
          <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
        <TestDataImporter />
      </div>
    </div>
  );
}

// Main app content that lives inside all providers
// Must be inside LayoutProvider to use useCommandPalette
function MainAppContent() {
  const [isTestDataModalOpen, setIsTestDataModalOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const chatInputRef = useRef<ChatInputHandle>(null);

  // Command palette hook (uses useLayout internally)
  const { isOpen: isPaletteOpen, close: closePalette } = useCommandPalette({
    onOpenSettings: () => setIsSettingsOpen(true),
    focusChatInput: () => chatInputRef.current?.focus(),
  });

  // Keyboard shortcut: Cmd+Shift+T to open test data importer
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey && e.shiftKey && e.key === 't') {
        e.preventDefault();
        setIsTestDataModalOpen(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <>
      <AppShell
        contextPanel={<EmployeeDetail />}
        onSettingsClick={() => setIsSettingsOpen(true)}
      >
        <ChatArea chatInputRef={chatInputRef} />
      </AppShell>
      <EmployeeEditModal />
      <ImportWizardModal />
      <Suspense fallback={null}>
        <SettingsPanel
          isOpen={isSettingsOpen}
          onClose={() => setIsSettingsOpen(false)}
        />
      </Suspense>
      <TestDataModal
        isOpen={isTestDataModalOpen}
        onClose={() => setIsTestDataModalOpen(false)}
      />
      <Suspense fallback={null}>
        <CommandPalette
          isOpen={isPaletteOpen}
          onClose={closePalette}
        />
      </Suspense>
      <Suspense fallback={null}>
        <UpgradePrompt />
      </Suspense>
    </>
  );
}

// Inner component that conditionally renders onboarding or main app
function AppContent() {
  const { isLoading, isCompleted } = useOnboarding();

  // Show loading state while checking onboarding status
  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-stone-50 to-stone-100">
        <div className="flex flex-col items-center gap-4">
          <div className="w-10 h-10 border-4 border-primary-200 border-t-primary-500 rounded-full animate-spin-slow" />
          <p className="text-stone-500">Loading...</p>
        </div>
      </div>
    );
  }

  // Show onboarding if not completed
  if (!isCompleted) {
    return <OnboardingFlow />;
  }

  // Main app after onboarding is complete
  return (
    <ErrorBoundary>
      <LayoutProvider>
        <ConversationProvider>
          <TrialProvider>
            <EmployeeProvider>
              <MainAppContent />
            </EmployeeProvider>
          </TrialProvider>
        </ConversationProvider>
      </LayoutProvider>
    </ErrorBoundary>
  );
}

function App() {
  return (
    <OnboardingProvider>
      <AppContent />
    </OnboardingProvider>
  );
}

export default App;
