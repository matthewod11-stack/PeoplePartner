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
import type { Employee } from '../lib/types';
import {
  listEmployeesWithRatings,
  type EmployeeWithLatestRating,
  type EmployeeFilter,
} from '../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

interface EmployeeContextValue {
  // Data
  employees: EmployeeWithLatestRating[];
  selectedEmployeeId: string | null;
  selectedEmployee: EmployeeWithLatestRating | null;
  totalCount: number;

  // Loading states
  isLoading: boolean;
  error: string | null;

  // Filters
  filter: EmployeeFilter;
  searchQuery: string;

  // Modal states
  isEditModalOpen: boolean;
  isImportWizardOpen: boolean;

  // Actions
  selectEmployee: (id: string | null) => void;
  setSearchQuery: (query: string) => void;
  setFilter: (filter: EmployeeFilter) => void;
  refreshEmployees: () => Promise<void>;
  openEditModal: () => void;
  closeEditModal: () => void;
  updateEmployeeInList: (updated: Employee) => void;
  openImportWizard: () => void;
  closeImportWizard: () => void;
}

// =============================================================================
// Context
// =============================================================================

const EmployeeContext = createContext<EmployeeContextValue | null>(null);

// =============================================================================
// Provider
// =============================================================================

interface EmployeeProviderProps {
  children: ReactNode;
}

export function EmployeeProvider({ children }: EmployeeProviderProps) {
  // Core state
  const [employees, setEmployees] = useState<EmployeeWithLatestRating[]>([]);
  const [selectedEmployeeId, setSelectedEmployeeId] = useState<string | null>(null);
  const [totalCount, setTotalCount] = useState(0);

  // Loading states
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [filter, setFilter] = useState<EmployeeFilter>({});
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState('');
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hasLoadedOnceRef = useRef(false);

  // Debounce search query (300ms delay)
  useEffect(() => {
    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }
    searchTimeoutRef.current = setTimeout(() => {
      setDebouncedSearchQuery(searchQuery);
    }, 300);

    return () => {
      if (searchTimeoutRef.current) {
        clearTimeout(searchTimeoutRef.current);
      }
    };
  }, [searchQuery]);

  // Modal states
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isImportWizardOpen, setIsImportWizardOpen] = useState(false);

  // Derived: currently selected employee
  const selectedEmployee = useMemo(
    () => employees.find((e) => e.id === selectedEmployeeId) ?? null,
    [employees, selectedEmployeeId]
  );

  // Fetch employees from backend
  const refreshEmployees = useCallback(async () => {
    // Only show loading skeleton on initial load
    // For search/filter updates, keep current list visible while fetching
    const isInitialLoad = !hasLoadedOnceRef.current;
    if (isInitialLoad) {
      setIsLoading(true);
    }
    setError(null);

    try {
      // Build filter with debounced search query
      const effectiveFilter: EmployeeFilter = {
        ...filter,
        search: debouncedSearchQuery || undefined,
      };

      const result = await listEmployeesWithRatings(effectiveFilter, 200, 0);
      setEmployees(result.employees);
      setTotalCount(result.total);
      hasLoadedOnceRef.current = true;
      setIsLoading(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load employees');
      setIsLoading(false);
    }
  }, [filter, debouncedSearchQuery]);

  // Select an employee
  const selectEmployee = useCallback((id: string | null) => {
    setSelectedEmployeeId(id);
  }, []);

  // Edit modal controls
  const openEditModal = useCallback(() => {
    setIsEditModalOpen(true);
  }, []);

  const closeEditModal = useCallback(() => {
    setIsEditModalOpen(false);
  }, []);

  // Update employee in local list (after edit)
  const updateEmployeeInList = useCallback((updated: Employee) => {
    setEmployees((prev) =>
      prev.map((emp) =>
        emp.id === updated.id ? { ...updated, latestRating: emp.latestRating } : emp
      )
    );
  }, []);

  // Import wizard controls
  const openImportWizard = useCallback(() => {
    setIsImportWizardOpen(true);
  }, []);

  const closeImportWizard = useCallback(() => {
    setIsImportWizardOpen(false);
  }, []);

  // Load employees on mount and when filters change
  useEffect(() => {
    refreshEmployees();
  }, [refreshEmployees]);

  // Context value
  const value: EmployeeContextValue = useMemo(
    () => ({
      employees,
      selectedEmployeeId,
      selectedEmployee,
      totalCount,
      isLoading,
      error,
      filter,
      searchQuery,
      isEditModalOpen,
      isImportWizardOpen,
      selectEmployee,
      setSearchQuery,
      setFilter,
      refreshEmployees,
      openEditModal,
      closeEditModal,
      updateEmployeeInList,
      openImportWizard,
      closeImportWizard,
    }),
    [
      employees,
      selectedEmployeeId,
      selectedEmployee,
      totalCount,
      isLoading,
      error,
      filter,
      searchQuery,
      isEditModalOpen,
      isImportWizardOpen,
      selectEmployee,
      refreshEmployees,
      openEditModal,
      closeEditModal,
      updateEmployeeInList,
      openImportWizard,
      closeImportWizard,
    ]
  );

  return (
    <EmployeeContext.Provider value={value}>
      {children}
    </EmployeeContext.Provider>
  );
}

// =============================================================================
// Hook
// =============================================================================

export function useEmployees() {
  const context = useContext(EmployeeContext);
  if (!context) {
    throw new Error('useEmployees must be used within an EmployeeProvider');
  }
  return context;
}

export default EmployeeContext;
