import { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import './LoadingContext.css';

interface LoadingState {
  active: boolean;
  message: string;
}

interface LoadingContextType {
  isLoading: boolean;
  loadingMessage: string;
  showLoading: (message?: string) => void;
  hideLoading: () => void;
}

const LoadingContext = createContext<LoadingContextType | undefined>(undefined);

export function LoadingProvider({ children }: { children: ReactNode }) {
  const [loadingState, setLoadingState] = useState<LoadingState>({
    active: false,
    message: 'Loading...',
  });

  const showLoading = useCallback((message = 'Loading...') => {
    setLoadingState({ active: true, message });
  }, []);

  const hideLoading = useCallback(() => {
    setLoadingState({ active: false, message: '' });
  }, []);

  return (
    <LoadingContext.Provider
      value={{
        isLoading: loadingState.active,
        loadingMessage: loadingState.message,
        showLoading,
        hideLoading,
      }}
    >
      {children}
      {loadingState.active && <LoadingOverlay message={loadingState.message} />}
    </LoadingContext.Provider>
  );
}

export function useLoading(): LoadingContextType {
  const context = useContext(LoadingContext);
  if (context === undefined) {
    throw new Error('useLoading must be used within a LoadingProvider');
  }
  return context;
}

interface LoadingOverlayProps {
  message: string;
}

function LoadingOverlay({ message }: LoadingOverlayProps) {
  return (
    <div className="loading-overlay">
      <div className="loading-content">
        <div className="loading-spinner">
          <div className="spinner-ring"></div>
        </div>
        <div className="loading-message">{message}</div>
      </div>
    </div>
  );
}

export default LoadingContext;
