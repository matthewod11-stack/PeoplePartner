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
import { categorizeError } from '../lib/error-utils';
import {
  listConversations,
  getConversation,
  updateConversation,
  deleteConversation as deleteConversationApi,
  searchConversations as searchConversationsApi,
  generateConversationTitle,
  sendChatMessageStreaming,
  getSystemPrompt,
  generateConversationSummary,
  saveConversationSummary,
  scanPii,
  createAuditEntry,
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
  retryMessage: (messageId: string) => Promise<void>;
  loadConversation: (id: string) => Promise<void>;
  startNewConversation: () => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  refreshConversations: () => Promise<void>;

  // PII redaction notification
  piiNotification: string | null;
  clearPiiNotification: () => void;
}

interface ConversationMessagesContextValue {
  messages: Message[];
  isLoading: boolean;
}

interface ConversationMetaContextValue {
  conversationId: string;
  currentTitle: string | null;
  piiNotification: string | null;
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
  retryMessage: (messageId: string) => Promise<void>;
  loadConversation: (id: string) => Promise<void>;
  startNewConversation: () => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  refreshConversations: () => Promise<void>;
  clearPiiNotification: () => void;
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
  // Audit logging state (refs to avoid re-renders)
  // ---------------------------------------------------------------------------
  const redactedMessageRef = useRef<string | null>(null);
  const employeeIdsRef = useRef<string[]>([]);
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

  const clearPiiNotification = useCallback(() => {
    setPiiNotification(null);
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
      if (redactionResult.had_pii) {
        messageContent = redactionResult.redacted_text;
        setPiiNotification(redactionResult.summary);
      }
    } catch (err) {
      console.error('[PII] Scan failed:', err);
      const shouldContinue = window.confirm(
        'PII scanning is temporarily unavailable. Your message may contain sensitive information (SSN, credit cards, etc.) that would normally be redacted.\n\nSend anyway?'
      );
      if (!shouldContinue) {
        setIsLoading(false);
        return;
      }
      // Continue with original content if user confirms
    }

    // Store redacted message for audit logging
    redactedMessageRef.current = messageContent;

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

    // Set up stream event listener
    let unlisten: UnlistenFn | null = null;

    // Stream inactivity timeout — resets on every chunk, fires error if stream stalls
    let streamTimeoutId: number | null = null;

    const resetStreamTimeout = () => {
      if (streamTimeoutId !== null) clearTimeout(streamTimeoutId);
      streamTimeoutId = window.setTimeout(() => {
        streamTimeoutId = null;
        flushBufferedChunkNow();
        if (unlisten) { unlisten(); unlisten = null; }

        const chatError = categorizeError(new Error('Response timed out — no data received for 30 seconds. The AI provider may be experiencing issues.'));
        chatError.originalContent = content;
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === assistantId ? { ...msg, content: '', error: chatError } : msg
          )
        );
        streamingMessageId.current = null;
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

      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === assistantId
            ? { ...msg, content: msg.content + chunkToApply }
            : msg
        )
      );
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

          // Get the full accumulated response before resetting
          const fullResponse = accumulatedResponseRef.current;

          // V2.1.4: Update message with verification result if present
          if (verification) {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === assistantId
                  ? { ...msg, verification }
                  : msg
              )
            );
          }

          // Create audit entry (fire-and-forget, don't block on errors)
          // Use conversationIdRef to avoid stale closure if user switched conversations during streaming
          createAuditEntry({
            conversation_id: conversationIdRef.current,
            request_redacted: redactedMessageRef.current ?? '',
            response_text: fullResponse,
            employee_ids_used: employeeIdsRef.current,
          }).catch((err) => {
            // Log but don't fail - audit is non-critical
            console.error('[Audit] Failed to create entry:', err);
          });

          // Reset refs for next message
          redactedMessageRef.current = null;
          employeeIdsRef.current = [];
          accumulatedResponseRef.current = '';

          streamingMessageId.current = null;
          setIsLoading(false);
        } else {
          // Accumulate response for audit logging
          accumulatedResponseRef.current += chunk;
          bufferedChunk += chunk;
          scheduleBufferedFlush();
        }
      });

      // Build message history for API
      const currentMessages = await new Promise<Message[]>((resolve) => {
        setMessages((prev) => {
          resolve(prev);
          return prev;
        });
      });

      const apiMessages: ChatMessage[] = currentMessages
        .slice(0, -1) // Exclude the empty assistant message
        .map((m) => ({
          role: m.role,
          content: m.content,
        }));

      // Build system prompt with context (prioritize selected employee if any)
      // V2.1.4: Now returns SystemPromptResult with aggregates for verification
      const promptResult = await getSystemPrompt(content, selectedEmployeeId);
      employeeIdsRef.current = promptResult.employee_ids_used;

      // Reset accumulated response for this message
      accumulatedResponseRef.current = '';

      // Start stream inactivity timeout (covers wait for first chunk)
      resetStreamTimeout();

      // Call Claude API with streaming
      // V2.1.4: Pass aggregates and query_type for answer verification
      await sendChatMessageStreaming(
        apiMessages,
        promptResult.system_prompt,
        promptResult.aggregates,
        promptResult.query_type
      );
    } catch (error) {
      clearStreamTimeout();
      flushBufferedChunkNow();

      // Categorize error for user-friendly display
      const chatError = categorizeError(error);
      chatError.originalContent = content;

      // Update assistant message with error state
      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === assistantId
            ? {
                ...msg,
                content: '',
                error: chatError,
              }
            : msg
        )
      );
      setIsLoading(false);
    } finally {
      flushBufferedChunkNow();

      if (unlisten) {
        unlisten();
      }
    }
  }, [conversationId]);

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
  }, []);

  // ---------------------------------------------------------------------------
  // Start a new conversation
  // ---------------------------------------------------------------------------
  const startNewConversation = useCallback(async () => {
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
  }, [messages, conversationId, refreshConversations]);

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
    }),
    [conversationId, currentTitle, piiNotification]
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
      retryMessage,
      loadConversation,
      startNewConversation,
      deleteConversation,
      setSearchQuery,
      refreshConversations,
      clearPiiNotification,
    }),
    [
      sendMessage,
      retryMessage,
      loadConversation,
      startNewConversation,
      deleteConversation,
      setSearchQuery,
      refreshConversations,
      clearPiiNotification,
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
