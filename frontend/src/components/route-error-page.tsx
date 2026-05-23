import { AlertTriangle, Home, RefreshCw } from 'lucide-react';
import { Link, isRouteErrorResponse, useRouteError } from 'react-router-dom';

import { Button } from '@/components/ui/button';

export function RouteErrorPage() {
  const error = useRouteError();
  const details = routeErrorDetails(error);

  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-rose-300/60 bg-card/90 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.14)]">
        <div className="flex flex-col gap-5 md:flex-row md:items-start">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-rose-500/10 text-rose-700 dark:text-rose-100">
            <AlertTriangle className="h-6 w-6" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Route error
            </p>
            <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
              {details.title}
            </h2>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
              {details.description}
            </p>
            {details.status ? (
              <p className="mt-3 text-xs font-medium uppercase tracking-[0.18em] text-rose-700 dark:text-rose-100">
                HTTP {details.status}
              </p>
            ) : null}
          </div>
        </div>
      </div>
      <div className="flex flex-wrap gap-3">
        <Button onClick={() => window.location.reload()} type="button">
          <RefreshCw className="h-4 w-4" />
          Retry
        </Button>
        <Button asChild variant="outline">
          <Link to="/">
            <Home className="h-4 w-4" />
            Overview
          </Link>
        </Button>
      </div>
    </section>
  );
}

function routeErrorDetails(error: unknown) {
  if (isRouteErrorResponse(error)) {
    return {
      status: error.status,
      title: error.statusText || 'Route request failed',
      description:
        typeof error.data === 'string' && error.data.trim()
          ? error.data
          : 'The route could not finish loading. Retry the page or return to the overview.',
    };
  }

  if (error instanceof Error) {
    return {
      status: null,
      title: 'Page rendering failed',
      description: error.message || 'The route raised an unexpected error.',
    };
  }

  return {
    status: null,
    title: 'Page rendering failed',
    description: 'The route raised an unexpected error.',
  };
}
