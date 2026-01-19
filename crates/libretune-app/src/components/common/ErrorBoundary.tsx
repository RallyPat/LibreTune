import { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

/**
 * Error boundary component that catches JavaScript errors anywhere in its
 * child component tree and displays a fallback UI instead of crashing.
 */
class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
    };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    this.setState({ errorInfo });
    this.props.onError?.(error, errorInfo);
  }

  handleReset = (): void => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
    });
  };

  handleReload = (): void => {
    window.location.reload();
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div style={{
          padding: '20px',
          margin: '20px',
          backgroundColor: 'rgba(255, 59, 59, 0.1)',
          border: '1px solid rgba(255, 59, 59, 0.3)',
          borderRadius: '8px',
          color: '#e0e0e0',
        }}>
          <h3 style={{ color: '#ff6b6b', marginTop: 0 }}>
            ⚠️ Something went wrong
          </h3>
          <p style={{ color: '#b0b0b0' }}>
            An error occurred while rendering this component.
          </p>
          <details style={{ marginBottom: '16px' }}>
            <summary style={{ cursor: 'pointer', color: '#888' }}>
              Error details
            </summary>
            <pre style={{
              backgroundColor: 'rgba(0, 0, 0, 0.3)',
              padding: '12px',
              borderRadius: '4px',
              overflow: 'auto',
              fontSize: '12px',
              color: '#ff8888',
            }}>
              {this.state.error ? this.state.error.toString() : 'Unknown error'}
              {this.state.errorInfo?.componentStack && (
                <>
                  {'\n\nComponent Stack:'}
                  {this.state.errorInfo.componentStack}
                </>
              )}
            </pre>
          </details>
          <div style={{ display: 'flex', gap: '12px' }}>
            <button
              onClick={this.handleReset}
              style={{
                padding: '8px 16px',
                backgroundColor: 'rgba(100, 100, 100, 0.4)',
                border: '1px solid rgba(255, 255, 255, 0.2)',
                borderRadius: '4px',
                color: '#e0e0e0',
                cursor: 'pointer',
              }}
            >
              Try Again
            </button>
            <button
              onClick={this.handleReload}
              style={{
                padding: '8px 16px',
                backgroundColor: 'rgba(59, 130, 246, 0.3)',
                border: '1px solid rgba(59, 130, 246, 0.5)',
                borderRadius: '4px',
                color: '#e0e0e0',
                cursor: 'pointer',
              }}
            >
              Reload Page
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

export default ErrorBoundary;
