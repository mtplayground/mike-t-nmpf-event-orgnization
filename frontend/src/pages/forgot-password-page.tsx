import { useState } from 'react';
import { Link } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuthStore } from '@/stores/auth-store';

export function ForgotPasswordPage() {
  const forgotPassword = useAuthStore((state) => state.forgotPassword);
  const clearError = useAuthStore((state) => state.clearError);
  const lastError = useAuthStore((state) => state.lastError);
  const [email, setEmail] = useState('');
  const [sent, setSent] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSubmitting(true);

    try {
      await forgotPassword(email);
      setSent(true);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : 'Unable to start password reset.';
      useAuthStore.setState({ lastError: message });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <RouteCard
      eyebrow="Reset"
      title="Request a password reset"
      description="The backend now issues short-lived, single-use password reset tokens. This page triggers that flow."
    >
      <form className="grid gap-5" onSubmit={handleSubmit}>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="forgot-email">
            Account email
          </label>
          <Input
            id="forgot-email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@example.com"
            required
          />
        </div>
        {sent ? (
          <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-700 dark:text-emerald-300">
            If that address exists, a reset email has been queued.
          </div>
        ) : null}
        {lastError ? (
          <div className="rounded-2xl border border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {lastError}
          </div>
        ) : null}
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Sending…' : 'Send reset email'}
          </Button>
          <p className="text-sm text-muted-foreground">
            Already have a token?{' '}
            <Link
              to="/auth/reset-password"
              className="text-primary hover:underline"
            >
              Reset password
            </Link>
          </p>
        </div>
      </form>
    </RouteCard>
  );
}
