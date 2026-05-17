import { useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuthStore } from '@/stores/auth-store';

export function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const login = useAuthStore((state) => state.login);
  const clearError = useAuthStore((state) => state.clearError);
  const lastError = useAuthStore((state) => state.lastError);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const redirectTo =
    (location.state as { redirectTo?: string } | null)?.redirectTo || '/host';

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSubmitting(true);

    try {
      await login({ email, password });
      navigate(redirectTo, { replace: true });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : 'Unable to sign in right now.';
      // Store-level error is used across the auth experience; this keeps the page explicit.
      useAuthStore.setState({ lastError: message });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <RouteCard
      eyebrow="Login"
      title="Sign back into the event workspace"
      description="Use your verified account to resume host or attendee flows. Refresh tokens are restored automatically after login."
    >
      <form className="grid gap-5" onSubmit={handleSubmit}>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="login-email">
            Email
          </label>
          <Input
            id="login-email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@example.com"
            required
          />
        </div>
        <div className="grid gap-2">
          <div className="flex items-center justify-between gap-3">
            <label className="text-sm font-medium" htmlFor="login-password">
              Password
            </label>
            <Link
              to="/auth/forgot-password"
              className="text-sm text-primary hover:underline"
            >
              Forgot password?
            </Link>
          </div>
          <Input
            id="login-password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            placeholder="••••••••"
            required
          />
        </div>
        {lastError ? (
          <div className="rounded-2xl border border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {lastError}
          </div>
        ) : null}
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Signing in…' : 'Sign in'}
          </Button>
          <p className="text-sm text-muted-foreground">
            Need an account?{' '}
            <Link to="/auth/register" className="text-primary hover:underline">
              Create one
            </Link>
          </p>
        </div>
      </form>
    </RouteCard>
  );
}
