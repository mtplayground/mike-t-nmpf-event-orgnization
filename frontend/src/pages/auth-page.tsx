import { Link } from 'react-router-dom';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';
import { useAuthStore } from '@/stores/auth-store';

export function AuthPage() {
  const session = useAuthStore((state) => state.session);

  return (
    <RouteCard
      eyebrow="Auth"
      title="Authentication flows are now connected"
      description="Register, sign in, verify email, and recover passwords against the live backend endpoints from this route family."
    >
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <AuthLink
          title="Sign in"
          body="Start a token-backed session and unlock protected routes."
          to="/auth/login"
        />
        <AuthLink
          title="Register"
          body="Create an account and continue into email verification."
          to="/auth/register"
        />
        <AuthLink
          title="Verify email"
          body="Submit or resend the verification token from email."
          to="/auth/verify-email"
        />
        <AuthLink
          title="Reset password"
          body="Request a reset email or complete the password reset flow."
          to="/auth/forgot-password"
        />
      </div>
      <div className="mt-5 rounded-2xl border border-border/60 bg-background/80 p-4 text-sm text-muted-foreground">
        {session ? (
          <>
            Current session:{' '}
            <span className="font-medium text-foreground">
              {session.user.email}
            </span>{' '}
            with host and attendee routes available.
          </>
        ) : (
          <>
            No active session. Protected routes redirect to login until the auth
            store restores or refreshes a valid token pair.
          </>
        )}
      </div>
    </RouteCard>
  );
}

type AuthLinkProps = {
  title: string;
  body: string;
  to: string;
};

function AuthLink({ title, body, to }: AuthLinkProps) {
  return (
    <div className="rounded-2xl border border-border/60 bg-background/80 p-4">
      <h3 className="text-sm font-semibold">{title}</h3>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
      <Button asChild className="mt-4">
        <Link to={to}>Open flow</Link>
      </Button>
    </div>
  );
}
