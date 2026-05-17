import { useMemo, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuthStore } from '@/stores/auth-store';

export function VerifyEmailPage() {
  const location = useLocation();
  const verifyEmail = useAuthStore((state) => state.verifyEmail);
  const resendVerification = useAuthStore((state) => state.resendVerification);
  const clearError = useAuthStore((state) => state.clearError);
  const lastError = useAuthStore((state) => state.lastError);
  const [token, setToken] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [resending, setResending] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const suggestedEmail = useMemo(
    () => (location.state as { email?: string } | null)?.email || '',
    [location.state],
  );

  async function handleVerify(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSuccessMessage(null);
    setSubmitting(true);

    try {
      await verifyEmail(token);
      setSuccessMessage('Email verified. You can sign in now.');
    } catch (error) {
      const message =
        error instanceof Error ? error.message : 'Verification failed.';
      useAuthStore.setState({ lastError: message });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleResend() {
    if (!suggestedEmail) {
      return;
    }

    clearError();
    setSuccessMessage(null);
    setResending(true);

    try {
      await resendVerification(suggestedEmail);
      setSuccessMessage(`Verification email re-sent to ${suggestedEmail}.`);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : 'Unable to resend verification.';
      useAuthStore.setState({ lastError: message });
    } finally {
      setResending(false);
    }
  }

  return (
    <RouteCard
      eyebrow="Verify"
      title="Confirm the email token"
      description="Paste the verification token from the email. Resend is available when you arrive here directly from registration."
    >
      <form className="grid gap-5" onSubmit={handleVerify}>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="verify-token">
            Verification token
          </label>
          <Input
            id="verify-token"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            placeholder="Paste the 64-character token"
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
            {submitting ? 'Verifying…' : 'Verify email'}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={!suggestedEmail || resending}
            onClick={handleResend}
          >
            {resending ? 'Re-sending…' : 'Resend verification'}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">
          Ready after verification?{' '}
          <Link to="/auth/login" className="text-primary hover:underline">
            Sign in
          </Link>
        </p>
      </form>
    </RouteCard>
  );
}
