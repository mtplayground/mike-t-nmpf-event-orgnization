import { RouteCard } from '@/components/route-card';

export function AuthPage() {
  return (
    <RouteCard
      eyebrow="Auth"
      title="Authentication route placeholder"
      description="Login, registration, password reset, and verification views will live here in later issues."
    >
      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-2xl border border-dashed border-border/70 p-4 text-sm text-muted-foreground">
          Planned flows: sign in, create account, verify email.
        </div>
        <div className="rounded-2xl border border-dashed border-border/70 p-4 text-sm text-muted-foreground">
          Shared providers already exist for query state and app-wide store
          usage.
        </div>
      </div>
    </RouteCard>
  );
}
