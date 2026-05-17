import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuthStore } from '@/stores/auth-store';

export function RegisterPage() {
  const navigate = useNavigate();
  const register = useAuthStore((state) => state.register);
  const clearError = useAuthStore((state) => state.clearError);
  const lastError = useAuthStore((state) => state.lastError);
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSubmitting(true);

    try {
      const result = await register({
        email,
        display_name: displayName,
        password,
      });
      navigate('/auth/verify-email', {
        replace: true,
        state: { email: result.email },
      });
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : 'Unable to create the account.';
      useAuthStore.setState({ lastError: message });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <RouteCard
      eyebrow="Register"
      title="Create your organizer account"
      description="Registration now talks to the backend flow and will queue a verification email before protected routes become available."
    >
      <form className="grid gap-5" onSubmit={handleSubmit}>
        <div className="grid gap-2">
          <label
            className="text-sm font-medium"
            htmlFor="register-display-name"
          >
            Display name
          </label>
          <Input
            id="register-display-name"
            autoComplete="name"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Mike T"
            required
          />
        </div>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="register-email">
            Email
          </label>
          <Input
            id="register-email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@example.com"
            required
          />
        </div>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="register-password">
            Password
          </label>
          <Input
            id="register-password"
            type="password"
            autoComplete="new-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            placeholder="At least 8 characters"
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
            {submitting ? 'Creating account…' : 'Create account'}
          </Button>
          <p className="text-sm text-muted-foreground">
            Already registered?{' '}
            <Link to="/auth/login" className="text-primary hover:underline">
              Sign in
            </Link>
          </p>
        </div>
      </form>
    </RouteCard>
  );
}
