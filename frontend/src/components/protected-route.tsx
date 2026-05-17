import type { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { useAuthStore } from '@/stores/auth-store';

type ProtectedRouteProps = {
  children: ReactNode;
};

export function ProtectedRoute({ children }: ProtectedRouteProps) {
  const initialized = useAuthStore((state) => state.initialized);
  const isRefreshing = useAuthStore((state) => state.isRefreshing);
  const session = useAuthStore((state) => state.session);
  const location = useLocation();

  if (!initialized || isRefreshing) {
    return (
      <RouteCard
        eyebrow="Secure"
        title="Checking your session"
        description="Protected pages wait for the auth store to restore or refresh your session before rendering."
      >
        <div className="rounded-2xl border border-dashed border-border/70 p-5 text-sm text-muted-foreground">
          Confirming your access token and user context.
        </div>
      </RouteCard>
    );
  }

  if (!session) {
    return (
      <Navigate
        to="/auth/login"
        replace
        state={{ redirectTo: `${location.pathname}${location.search}` }}
      />
    );
  }

  return <>{children}</>;
}
