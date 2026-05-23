import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';

import { Button } from '@/components/ui/button';

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  error: Error | null;
};

export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = {
    error: null,
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Unhandled frontend error', error, errorInfo);
  }

  render() {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <main className="min-h-screen bg-background px-4 py-10 text-foreground">
        <div className="mx-auto flex max-w-2xl flex-col items-start gap-5 rounded-[28px] border border-rose-300/60 bg-card p-6 shadow-[0_24px_80px_rgba(15,23,42,0.16)]">
          <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-rose-500/10 text-rose-700 dark:text-rose-100">
            <AlertTriangle className="h-6 w-6" />
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Application error
            </p>
            <h1 className="mt-3 font-serif text-3xl tracking-tight">
              Something interrupted the page.
            </h1>
            <p className="mt-3 text-sm leading-7 text-muted-foreground">
              The interface stopped rendering before it could recover. Reload
              the app to restore a clean session.
            </p>
          </div>
          <Button onClick={() => window.location.reload()} type="button">
            <RefreshCw className="h-4 w-4" />
            Reload app
          </Button>
        </div>
      </main>
    );
  }
}
