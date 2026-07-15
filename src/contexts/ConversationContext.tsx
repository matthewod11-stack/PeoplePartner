// People Partner - Conversation Context
// Manages chat state, conversation history, and auto-persistence

import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { Message } from '../lib/types';
import { categorizeError, isCancelledError } from '../lib/error-utils';
import {
  appendChunk,
  setMessageError,
  setMessageVerification,
  toApiMessages,
  resolveRedaction,
} from '../lib/message-updates';
import {
  listConversations,
  getConversation,
  updateConversation,
  deleteConversation as deleteConversationApi,
  searchConversations as searchConversationsApi,
  generateConversationTitle,
  sendChatMessageStreaming,
  cancelStream,
  getSystemPrompt,
  generateConversationSummary,
  saveConversationSummary,
  scanPii,
  type ConversationListItem,
  type ChatMessage,
  type StreamChunk,
} from '../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

interface ConversationContextValue {
  // Current conversation state
  messages: Message[];
  conversationId: string;
  isLoading: boolean;
  currentTitle: string | null;

  // Conversation list for sidebar
  conversations: ConversationListItem[];
  isLoadingList: boolean;
  listError: string | null;

  // Search
  searchQuery: string;
  isSearching: boolean;

  // Actions
  sendMessage: (content: string, selectedEmployeeId?: string | null) => Promise<void>;
  stopStreaming: () => void;
  retryMessage: (messageId: string) => Promise<void>;
  loadConversation: (id: string) => Promise<void>;
  startNewConversation: () => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  refreshConversations: () => Promise<void>;

  // PII redaction notification (success: text was scanned, redactions surfaced)
  piiNotification: string | null;
  clearPiiNotification: () => void;

  // PII scan failure (fail-closed: message was NOT sent)
  piiScanError: string | null;
  clearPiiScanError: () => void;

  // One-time notice when memory summaries are unavailable (#108)
  memoryNotice: string | null;
  clearMemoryNotice: () => void;
}

interface ConversationMessagesContextValue {
  messages: Message[];
  isLoading: boolean;
}

interface ConversationMetaContextValue {
  conversationId: string;
  currentTitle: string | null;
  piiNotification: string | null;
  piiScanError: string | null;
  memoryNotice: string | null;
}

interface ConversationDirectoryContextValue {
  conversations: ConversationListItem[];
  isLoadingList: boolean;
  listError: string | null;
  searchQuery: string;
  isSearching: boolean;
}

interface ConversationActionsContextValue {
  sendMessage: (content: string, selectedEmployeeId?: string | null) => Promise<void>;
  stopStreaming: () => void;
  retryMessage: (messageId: string) => Promise<void>;
  loadConversation: (id: string) => Promise<void>;
  startNewConversation: () => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  refreshConversations: () => Promise<void>;
  clearPiiNotification: () => void;
  clearPiiScanError: () => void;
  clearMemoryNotice: () => void;
}

// =============================================================================
// Context
// =============================================================================

const ConversationContext = createContext<ConversationContextValue | null>(null);
const ConversationMessagesContext = createContext<ConversationMessagesContextValue | null>(null);
const ConversationMetaContext = createContext<ConversationMetaContextValue | null>(null);
const ConversationDirectoryContext = createContext<ConversationDirectoryContextValue | null>(null);
const ConversationActionsContext = createContext<ConversationActionsContextValue | null>(null);

// =============================================================================
// Constants
// =============================================================================

/** Minimum exchanges before generating a summary */
const MIN_EXCHANGES_FOR_SUMMARY = 2;

/** Debounce delay for search (ms) */
const SEARCH_DEBOUNCE_MS = 300;

/** Streaming message update interval (ms) */
const STREAM_UPDATE_INTERVAL_MS = 100;

/** Stream inactivity timeout — error if no chunks received for this long (ms) */
const STREAM_TIMEOUT_MS = 30_000;

// =============================================================================
// Provider
// =============================================================================

interface ConversationProviderProps {
  children: ReactNode;
}

export function ConversationProvider({ children }: ConversationProviderProps) {
  // ---------------------------------------------------------------------------
  // Current conversation state
  // ---------------------------------------------------------------------------
  const [messages, setMessages] = useState<Message[]>([]);
  const [conversationId, setConversationId] = useState<string>(() => crypto.randomUUID());
  const [isLoading, setIsLoading] = useState(false);
  const [currentTitle, setCurrentTitle] = useState<string | null>(null);
  const streamingMessageId = useRef<string | null>(null);
  // #147: the client-generated id of the in-flight stream, so the UI can
  // cancel it (Stop button, conversation switch, unmount). Null when idle.
  const activeStreamIdRef = useRef<string | null>(null);

  // Ref to track current conversationId for async/stream handlers (avoids stale closures)
  const conversationIdRef = useRef(conversationId);
  useEffect(() => {
    conversationIdRef.current = conversationId;
  }, [conversationId]);

  // Track if title has been generated for current conversation
  const titleGenerated = useRef<Set<string>>(new Set());

  // Track previous isLoading for auto-save trigger
  const prevIsLoading = useRef(false);

  // ---------------------------------------------------------------------------
  // Streaming accumulation state (ref to avoid re-renders). Audit logging
  // moved backend-side (#112) — the chat seam writes the row itself.
  // ---------------------------------------------------------------------------
  const accumulatedResponseRef = useRef<string>('');

  // ---------------------------------------------------------------------------
  // Conversation list state
  // ---------------------------------------------------------------------------
  const [conversations, setConversations] = useState<ConversationListItem[]>([]);
  const [isLoadingList, setIsLoadingList] = useState(true);
  const [listError, setListError] = useState<string | null>(null);

  // ---------------------------------------------------------------------------
  // Search state
  // ---------------------------------------------------------------------------
  const [searchQuery, setSearchQueryState] = useState('');
  const [isSearching, setIsSearching] = useState(false);
  const searchTimeoutRef = useRef<number | null>(null);

  // ---------------------------------------------------------------------------
  // PII notification state
  // ---------------------------------------------------------------------------
  const [piiNotification, setPiiNotification] = useState<string | null>(null);
  // Separate state for fail-closed scan errors. Persistent (no auto-dismiss);
  // distinct from the redaction-success summary so the renderer can style it
  // as an error and the user understands the message was NOT sent.
  const [piiScanError, setPiiScanError] = useState<string | null>(null);

  const clearPiiNotification = useCallback(() => {
    setPiiNotification(null);
  }, []);

  const clearPiiScanError = useCallback(() => {
    setPiiScanError(null);
  }, []);

  // ---------------------------------------------------------------------------
  // Memory-unavailable notice (#108)
  // ---------------------------------------------------------------------------
  // Summary generation needs an API key for the active provider. Trial users
  // (and misconfigured BYOK setups) have none — instead of swallowing the
  // failure in a console.warn, surface it once per session so the user knows
  // why cross-conversation memory is off and how to turn it on.
  const [memoryNotice, setMemoryNotice] = useState<string | null>(null);
  const memoryNoticeShown = useRef(false);

  const clearMemoryNotice = useCallback(() => {
    setMemoryNotice(null);
  }, []);

  // ---------------------------------------------------------------------------
  // Fetch conversation list
  // ---------------------------------------------------------------------------
  const refreshConversations = useCallback(async () => {
    setIsLoadingList(true);
    setListError(null);

    try {
      const result = await listConversations(50, 0);
      setConversations(result);
    } catch (err) {
      setListError(err instanceof Error ? err.message : 'Failed to load conversations');
    } finally {
      setIsLoadingList(false);
    }
  }, []);

  // Load conversations on mount
  useEffect(() => {
    refreshConversations();
  }, [refreshConversations]);

  // ---------------------------------------------------------------------------
  // Search with debounce
  // ---------------------------------------------------------------------------
  const performSearch = useCallback(async (query: string) => {
    if (!query.trim()) {
      // Empty query - show recent conversations
      await refreshConversations();
      return;
    }

    setIsSearching(true);
    try {
      const results = await searchConversationsApi(query, 20);
      setConversations(results);
    } catch (err) {
      console.error('Search failed:', err);
      // On error, fall back to showing recent
      await refreshConversations();
    } finally {
      setIsSearching(false);
    }
  }, [refreshConversations]);

  const setSearchQuery = useCallback((query: string) => {
    setSearchQueryState(query);

    // Clear existing timeout
    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }

    // Debounce search
    searchTimeoutRef.current = window.setTimeout(() => {
      performSearch(query);
    }, SEARCH_DEBOUNCE_MS);
  }, [performSearch]);

  // ---------------------------------------------------------------------------
  // Auto-save conversation after assistant completes
  // ---------------------------------------------------------------------------
  useEffect(() => {
    // Detect transition from loading to not loading (assistant done)
    if (prevIsLoading.current && !isLoading && messages.length > 0) {
      // Save conversation to database
      const saveConversation = async () => {
        try {
          const messagesJson = JSON.stringify(messages);
          await updateConversation(conversationId, {
            messages_json: messagesJson,
            title: currentTitle ?? undefined,
          });
          console.log('[Conversation] Auto-saved after assistant response');

          // Refresh list to show updated timestamp
          await refreshConversations();
        } catch (err) {
          console.error('[Conversation] Auto-save failed:', err);
          // Surface to user so they know to manually save or not close the app
          window.dispatchEvent(new CustomEvent('conversation-save-error', {
            detail: { error: String(err) }
          }));
        }
      };

      saveConversation();
    }

    prevIsLoading.current = isLoading;
  }, [isLoading, messages, conversationId, currentTitle, refreshConversations]);

  // ---------------------------------------------------------------------------
  // Auto-generate title after first exchange
  // ---------------------------------------------------------------------------
  useEffect(() => {
    // Check if we have a complete first exchange (user + assistant)
    const userMessages = messages.filter(m => m.role === 'user');
    const assistantMessages = messages.filter(m => m.role === 'assistant' && m.content.length > 0);

    if (
      userMessages.length >= 1 &&
      assistantMessages.length >= 1 &&
      !currentTitle &&
      !titleGenerated.current.has(conversationId) &&
      !isLoading
    ) {
      const generateTitle = async () => {
        try {
          titleGenerated.current.add(conversationId);
          const firstMessage = userMessages[0].content;
          const title = await generateConversationTitle(firstMessage);
          setCurrentTitle(title);

          // Save title to database
          await updateConversation(conversationId, { title });
          console.log('[Conversation] Title generated:', title);

          // Refresh list to show new title
          await refreshConversations();
        } catch (err) {
          console.error('[Conversation] Title generation failed:', err);
        }
      };

      generateTitle();
    }
  }, [messages, conversationId, currentTitle, isLoading, refreshConversations]);

  // ---------------------------------------------------------------------------
  // Send message to Claude
  // ---------------------------------------------------------------------------
  const sendMessage = useCallback(async (content: string, selectedEmployeeId?: string | null) => {
    // Scan for PII and redact if found
    let messageContent = content;
    try {
      const redactionResult = await scanPii(content);
      const resolved = resolveRedaction(content, redactionResult);
      messageContent = resolved.content;
      if (redactionResult.had_pii) {
        setPiiNotification(resolved.notification);
      }
    } catch (err) {
      console.error('[PII] Scan failed:', err);
      // Fail closed: privacy is the product. We do not let the user opt out
      // of a redaction step that may have caught (e.g.) an SSN. The message
      // is not sent; the input retains its content for the user to retry.
      setPiiScanError(
        'Privacy scanner unavailable. Your message was not sent. Please try again in a moment.'
      );
      setIsLoading(false);
      return;
    }

    // Add user message (with potentially redacted content)
    const userMessage: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: messageContent,
      timestamp: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, userMessage]);
    setIsLoading(true);

    // Create empty assistant message for streaming
    const assistantId = crypto.randomUUID();
    streamingMessageId.current = assistantId;
    const assistantMessage: Message = {
      id: assistantId,
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, assistantMessage]);

    // #147: client-generated id for this stream so the user can cancel it.
    const streamId = crypto.randomUUID();
    activeStreamIdRef.current = streamId;

    // Set up stream event listeners
    let unlisten: UnlistenFn | null = null;
    // #147: separate listener for the backend's cancelled signal (chat.rs emits
    // "chat-stream-cancelled" and NOT a normal `done`, so the reset lives here).
    let unlistenCancel: UnlistenFn | null = null;

    // Stream inactivity timeout — resets on every chunk, fires error if stream stalls
    let streamTimeoutId: number | null = null;

    const resetStreamTimeout = () => {
      if (streamTimeoutId !== null) clearTimeout(streamTimeoutId);
      streamTimeoutId = window.setTimeout(() => {
        streamTimeoutId = null;
        flushBufferedChunkNow();
        if (unlisten) { unlisten(); unlisten = null; }
        if (unlistenCancel) { unlistenCancel(); unlistenCancel = null; }

        const chatError = categorizeError(new Error('Response timed out — no data received for 30 seconds. The AI provider may be experiencing issues.'));
        chatError.originalContent = content;
        setMessages((prev) => setMessageError(prev, assistantId, chatError));
        streamingMessageId.current = null;
        activeStreamIdRef.current = null;
        setIsLoading(false);
      }, STREAM_TIMEOUT_MS);
    };

    const clearStreamTimeout = () => {
      if (streamTimeoutId !== null) { clearTimeout(streamTimeoutId); streamTimeoutId = null; }
    };

    // Buffer stream chunks to avoid 20-50 re-renders/second during streaming
    let bufferedChunk = '';
    let flushTimeoutId: number | null = null;

    const flushBufferedChunk = () => {
      if (!bufferedChunk) {
        return;
      }

      const chunkToApply = bufferedChunk;
      bufferedChunk = '';

      setMessages((prev) => appendChunk(prev, assistantId, chunkToApply));
    };

    const scheduleBufferedFlush = () => {
      if (flushTimeoutId !== null) {
        return;
      }

      flushTimeoutId = window.setTimeout(() => {
        flushTimeoutId = null;
        flushBufferedChunk();
      }, STREAM_UPDATE_INTERVAL_MS);
    };

    const flushBufferedChunkNow = () => {
      if (flushTimeoutId !== null) {
        clearTimeout(flushTimeoutId);
        flushTimeoutId = null;
      }
      flushBufferedChunk();
    };

    try {
      unlisten = await listen<StreamChunk>('chat-stream', (event) => {
        const { chunk, done, verification } = event.payload;
        resetStreamTimeout();

        if (done) {
          clearStreamTimeout();
          // Ensure any buffered chunks are rendered before final message updates
          flushBufferedChunkNow();

          // V2.1.4: Update message with verification result if present
          if (verification) {
            setMessages((prev) => setMessageVerification(prev, assistantId, verification));
          }

          // #112: the audit row is now written backend-side at the chat seam
          // (every attempt, including errors/cancels the old frontend
          // fire-and-forget here silently skipped).

          // Reset refs for next message
          accumulatedResponseRef.current = '';

          streamingMessageId.current = null;
          activeStreamIdRef.current = null;
          setIsLoading(false);
        } else {
          // Accumulate response for audit logging
          accumulatedResponseRef.current += chunk;
          bufferedChunk += chunk;
          scheduleBufferedFlush();
        }
      });

      // #147: user hit Stop (or we cancelled on switch/unmount). The backend
      // dropped the upstream connection and emitted this instead of `done`.
      // Finalize cleanly: keep whatever streamed so far, reset streaming state.
      unlistenCancel = await listen('chat-stream-cancelled', () => {
        clearStreamTimeout();
        flushBufferedChunkNow();
        accumulatedResponseRef.current = '';
        streamingMessageId.current = null;
        activeStreamIdRef.current = null;
        setIsLoading(false);
      });

      // Build message history for API
      const currentMessages = await new Promise<Message[]>((resolve) => {
        setMessages((prev) => {
          resolve(prev);
          return prev;
        });
      });

      const apiMessages: ChatMessage[] = toApiMessages(currentMessages);

      // Build system prompt with context (prioritize selected employee if any)
      // V2.1.4: Now returns SystemPromptResult with aggregates for verification
      const promptResult = await getSystemPrompt(content, selectedEmployeeId);

      // Reset accumulated response for this message
      accumulatedResponseRef.current = '';

      // Start stream inactivity timeout (covers wait for first chunk)
      resetStreamTimeout();

      // Call Claude API with streaming
      // V2.1.4: Pass aggregates and query_type for answer verification
      // #112: conversation id + employee ids ride along so the backend's
      // audit row (written at the chat seam) is fully attributed. Use the
      // ref to avoid a stale closure if the user switches conversations.
      await sendChatMessageStreaming(
        apiMessages,
        promptResult.system_prompt,
        promptResult.aggregates,
        promptResult.query_type,
        conversationIdRef.current,
        promptResult.employee_ids_used,
        streamId
      );
    } catch (error) {
      clearStreamTimeout();
      flushBufferedChunkNow();

      if (isCancelledError(error)) {
        // #147: Stop is a user action, not a failure. The backend rejects the
        // invoke with ChatError::Cancelled *in addition to* emitting
        // "chat-stream-cancelled" — without this branch the rejection would
        // decorate the partial message with a generic error + retry chip.
        // Finalize quietly (idempotent with the event handler, whichever
        // lands first): keep the partial text, reset streaming state.
        accumulatedResponseRef.current = '';
        streamingMessageId.current = null;
        setIsLoading(false);
      } else {
        // Categorize error for user-friendly display
        const chatError = categorizeError(error);
        chatError.originalContent = content;

        // Update assistant message with error state
        setMessages((prev) => setMessageError(prev, assistantId, chatError));
        setIsLoading(false);
      }
    } finally {
      flushBufferedChunkNow();

      if (unlisten) {
        unlisten();
      }
      if (unlistenCancel) {
        unlistenCancel();
      }
      // #147: if this send is still the active stream (e.g. it errored without
      // a done/cancelled event), clear the id so a later Stop/switch doesn't
      // try to cancel a dead stream.
      if (activeStreamIdRef.current === streamId) {
        activeStreamIdRef.current = null;
      }
    }
  }, [conversationId]);

  // ---------------------------------------------------------------------------
  // Stop the in-flight stream (#147). Drives the UI reset via the backend's
  // "chat-stream-cancelled" event rather than mutating state here, so the same
  // path handles Stop, conversation switch, and unmount. Safe when idle.
  // ---------------------------------------------------------------------------
  const stopStreaming = useCallback(() => {
    const id = activeStreamIdRef.current;
    if (!id) return;
    cancelStream(id).catch((err) => {
      console.error('[Conversation] Failed to cancel stream:', err);
    });
  }, []);

  // #147: cancel any in-flight stream if the provider unmounts, so an
  // abandoned stream stops billing instead of running to completion.
  useEffect(() => {
    return () => {
      const id = activeStreamIdRef.current;
      if (id) {
        cancelStream(id).catch(() => {});
      }
    };
  }, []);

  // ---------------------------------------------------------------------------
  // Retry a failed message
  // ---------------------------------------------------------------------------
  const retryMessage = useCallback(async (messageId: string) => {
    // Find the failed message
    const failedMessage = messages.find((m) => m.id === messageId && m.error);
    if (!failedMessage?.error?.originalContent) {
      console.warn('[Conversation] Cannot retry: no original content found');
      return;
    }

    const originalContent = failedMessage.error.originalContent;

    // Remove the failed assistant message
    setMessages((prev) => prev.filter((m) => m.id !== messageId));

    // Resend the original content (note: selectedEmployeeId context is lost on retry)
    await sendMessage(originalContent);
  }, [messages, sendMessage]);

  // ---------------------------------------------------------------------------
  // Load a conversation from database
  // ---------------------------------------------------------------------------
  const loadConversation = useCallback(async (id: string) => {
    stopStreaming(); // #147: abandon any in-flight stream when switching away
    try {
      const conversation = await getConversation(id);

      // Parse messages from JSON
      const loadedMessages: Message[] = conversation.messages_json
        ? JSON.parse(conversation.messages_json)
        : [];

      setMessages(loadedMessages);
      setConversationId(id);
      setCurrentTitle(conversation.title ?? null);

      // Mark title as generated if it exists
      if (conversation.title) {
        titleGenerated.current.add(id);
      }

      console.log('[Conversation] Loaded:', id, 'with', loadedMessages.length, 'messages');
    } catch (err) {
      console.error('[Conversation] Failed to load:', err);
      throw err;
    }
  }, [stopStreaming]);

  // ---------------------------------------------------------------------------
  // Start a new conversation
  // ---------------------------------------------------------------------------
  const startNewConversation = useCallback(async () => {
    stopStreaming(); // #147: abandon any in-flight stream when switching away
    // Generate summary if current conversation has enough content
    const userMessages = messages.filter(m => m.role === 'user');
    const assistantMessages = messages.filter(m => m.role === 'assistant' && m.content.length > 0);
    const exchanges = Math.min(userMessages.length, assistantMessages.length);

    if (exchanges >= MIN_EXCHANGES_FOR_SUMMARY) {
      try {
        console.log('[Memory] Generating summary for conversation:', conversationId);
        const messagesJson = JSON.stringify(messages);
        const summary = await generateConversationSummary(messagesJson);
        await saveConversationSummary(conversationId, summary);
        console.log('[Memory] Summary saved:', summary.substring(0, 80) + '...');
      } catch (err) {
        console.warn('[Memory] Summary generation failed:', err);
        // MemoryError::NoApiKey serializes as "API key not configured" —
        // surface it once per session instead of silently losing memory (#108).
        if (!memoryNoticeShown.current && String(err).includes('API key not configured')) {
          memoryNoticeShown.current = true;
          setMemoryNotice(
            'Conversation memory is off: no API key is configured for your AI provider. ' +
              'Add one in Settings — or memory unlocks with a license after purchase.'
          );
        }
      }
    }

    // Clear state for new conversation
    setMessages([]);
    setCurrentTitle(null);
    const newId = crypto.randomUUID();
    setConversationId(newId);

    console.log('[Conversation] Started new conversation:', newId);

    // Refresh list to show any saved conversation
    await refreshConversations();
  }, [messages, conversationId, refreshConversations, stopStreaming]);

  // ---------------------------------------------------------------------------
  // Delete a conversation
  // ---------------------------------------------------------------------------
  const deleteConversation = useCallback(async (id: string) => {
    try {
      await deleteConversationApi(id);
      console.log('[Conversation] Deleted:', id);

      // If deleting current conversation, start a new one
      if (id === conversationId) {
        setMessages([]);
        setCurrentTitle(null);
        setConversationId(crypto.randomUUID());
      }

      // Refresh list
      await refreshConversations();
    } catch (err) {
      console.error('[Conversation] Failed to delete:', err);
      throw err;
    }
  }, [conversationId, refreshConversations]);

  // ---------------------------------------------------------------------------
  // Context value
  // ---------------------------------------------------------------------------
  const messagesValue = useMemo<ConversationMessagesContextValue>(
    () => ({
      messages,
      isLoading,
    }),
    [messages, isLoading]
  );

  const metaValue = useMemo<ConversationMetaContextValue>(
    () => ({
      conversationId,
      currentTitle,
      piiNotification,
      piiScanError,
      memoryNotice,
    }),
    [conversationId, currentTitle, piiNotification, piiScanError, memoryNotice]
  );

  const directoryValue = useMemo<ConversationDirectoryContextValue>(
    () => ({
      conversations,
      isLoadingList,
      listError,
      searchQuery,
      isSearching,
    }),
    [conversations, isLoadingList, listError, searchQuery, isSearching]
  );

  const actionsValue = useMemo<ConversationActionsContextValue>(
    () => ({
      sendMessage,
      stopStreaming,
      retryMessage,
      loadConversation,
      startNewConversation,
      deleteConversation,
      setSearchQuery,
      refreshConversations,
      clearPiiNotification,
      clearPiiScanError,
      clearMemoryNotice,
    }),
    [
      sendMessage,
      stopStreaming,
      retryMessage,
      loadConversation,
      startNewConversation,
      deleteConversation,
      setSearchQuery,
      refreshConversations,
      clearPiiNotification,
      clearPiiScanError,
      clearMemoryNotice,
    ]
  );

  // Backward-compatible aggregate context for existing consumers.
  const value = useMemo<ConversationContextValue>(
    () => ({
      ...messagesValue,
      ...metaValue,
      ...directoryValue,
      ...actionsValue,
    }),
    [messagesValue, metaValue, directoryValue, actionsValue]
  );

  return (
    <ConversationActionsContext.Provider value={actionsValue}>
      <ConversationMetaContext.Provider value={metaValue}>
        <ConversationDirectoryContext.Provider value={directoryValue}>
          <ConversationMessagesContext.Provider value={messagesValue}>
            <ConversationContext.Provider value={value}>
              {children}
            </ConversationContext.Provider>
          </ConversationMessagesContext.Provider>
        </ConversationDirectoryContext.Provider>
      </ConversationMetaContext.Provider>
    </ConversationActionsContext.Provider>
  );
}

// =============================================================================
// Hook
// =============================================================================

export function useConversationMessages() {
  const context = useContext(ConversationMessagesContext);
  if (!context) {
    throw new Error('useConversationMessages must be used within a ConversationProvider');
  }
  return context;
}

export function useConversationMeta() {
  const context = useContext(ConversationMetaContext);
  if (!context) {
    throw new Error('useConversationMeta must be used within a ConversationProvider');
  }
  return context;
}

export function useConversationDirectory() {
  const context = useContext(ConversationDirectoryContext);
  if (!context) {
    throw new Error('useConversationDirectory must be used within a ConversationProvider');
  }
  return context;
}

export function useConversationActions() {
  const context = useContext(ConversationActionsContext);
  if (!context) {
    throw new Error('useConversationActions must be used within a ConversationProvider');
  }
  return context;
}

export function useConversations() {
  const context = useContext(ConversationContext);
  if (!context) {
    throw new Error('useConversations must be used within a ConversationProvider');
  }
  return context;
}
