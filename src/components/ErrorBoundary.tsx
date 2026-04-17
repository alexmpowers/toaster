import React from "react";

interface ErrorBoundaryProps {
  children: React.ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

// ErrorBoundary must be a class component — React does not support
// error boundaries with hooks. Fallback text is hardcoded in English
// because i18n hooks (useTranslation) cannot be used in class components,
// and this is an emergency fallback that should always be readable.
class ErrorBoundary extends React.Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.error("ErrorBoundary caught an error:", error);
    console.error("Component stack:", errorInfo.componentStack);
  }

  handleReload = (): void => {
    window.location.reload();
  };

  render(): React.ReactNode {
    if (this.state.hasError) {
      const title = "Something went wrong";
      const reloadLabel = "Reload";
      return (
        <div className="flex flex-col items-center justify-center h-full bg-neutral-900 text-neutral-100 p-8 gap-4">
          <h2 className="text-lg font-semibold">{title}</h2>
          <p className="text-sm text-neutral-400 text-center max-w-md">
            {this.state.error?.message || "An unexpected error occurred."}
          </p>
          <button
            onClick={this.handleReload}
            className="mt-2 px-4 py-2 bg-neutral-700 hover:bg-neutral-600 text-neutral-100 rounded-lg text-sm font-medium transition-colors"
          >
            {reloadLabel}
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
