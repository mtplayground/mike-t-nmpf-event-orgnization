import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuthStore } from '@/stores/auth-store';

export function ResetPasswordPage() {
  const navigate = useNavigate();
  const resetPassword = useAuthStore((state) => state.resetPassword);
  const clearError = useAuthStore((state) => state.clearError);
  const lastError = useAuthStore((state) => state.lastError);
  const [token, setToken] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSuccessMessage(null);
    setSubmitting(true);

    try {
      await resetPassword(token, password);
      setSuccessMessage('Password updated. Redirecting to sign in…');
      window.setTimeout(() => navigate('/auth/login', { replace: true }), 900);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : 'Unable to reset the password.';
      useAuthStore.setState({ lastError: message });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <RouteCard
      eyebrow="Reset"
      title="Set a new password"
      description="Use the reset token from your email to rotate credentials and revoke active refresh tokens."
    >
      <form className="grid gap-5" onSubmit={handleSubmit}>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="reset-token">
            Reset token
          </label>
          <Input
            id="reset-token"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            placeholder="Paste the 64-character token"
            required
          />
        </div>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="reset-password">
            New password
          </label>
          <Input
            id="reset-password"
            type="password"
            autoComplete="new-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            placeholder="At least 8 characters"
            required
          />
        </div>
        {successMessage ? (
          <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-700 dark:text-emerald-300">
            {successMessage}
          </div>
        ) : null}
        {lastError ? (
          <div className="rounded-2xl border border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {lastError}
          </div>
        ) : null}
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Resetting…' : 'Reset password'}
          </Button>
          <p className="text-sm text-muted-foreground">
            Need a new token?{' '}
            <Link
              to="/auth/forgot-password"
              className="text-primary hover:underline"
            >
              Request reset email
            </Link>
          </p>
        </div>
      </form>
    </RouteCard>
  );
}
