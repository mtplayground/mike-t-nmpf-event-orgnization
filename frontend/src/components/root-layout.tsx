import { MoonStar, SunMedium } from 'lucide-react';
import { NavLink, Outlet } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { useAppStore } from '@/stores/app-store';
import { useAuthStore } from '@/stores/auth-store';

const routes = [
  { to: '/', label: 'Overview' },
  { to: '/auth', label: 'Auth' },
  { to: '/profile', label: 'Profile' },
  { to: '/host', label: 'Host' },
  { to: '/attendee', label: 'Attendee' },
];

export function RootLayout() {
  const apiBaseUrl = useAppStore((state) => state.apiBaseUrl);
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  const session = useAuthStore((state) => state.session);
  const logout = useAuthStore((state) => state.logout);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="absolute inset-x-0 top-0 -z-10 h-[28rem] bg-[radial-gradient(circle_at_top,_rgba(255,126,54,0.28),_transparent_35%),radial-gradient(circle_at_right,_rgba(18,165,148,0.2),_transparent_30%)]" />
      <div className="container py-8 md:py-12">
        <header className="mb-10 flex flex-col gap-6 rounded-[28px] border border-border/60 bg-card/75 px-6 py-5 shadow-[0_30px_80px_rgba(15,23,42,0.12)] backdrop-blur md:flex-row md:items-center md:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Mike T NMPF
            </p>
            <h1 className="mt-2 font-serif text-3xl tracking-tight md:text-4xl">
              Event platform frontend shell
            </h1>
          </div>
          <div className="flex flex-col items-start gap-3 md:items-end">
            <nav className="flex flex-wrap gap-2">
              {routes.map((route) => (
                <NavLink
                  key={route.to}
                  to={route.to}
                  className={({ isActive }) =>
                    [
                      'rounded-full px-4 py-2 text-sm font-medium transition',
                      isActive
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-secondary text-secondary-foreground hover:bg-accent hover:text-accent-foreground',
                    ].join(' ')
                  }
                >
                  {route.label}
                </NavLink>
              ))}
            </nav>
            <div className="flex items-center gap-3">
              <span className="rounded-full border border-border/60 bg-background/70 px-3 py-1 text-xs text-muted-foreground">
                {session
                  ? `Signed in as ${session.user.display_name}`
                  : 'Guest mode'}
              </span>
              <span className="rounded-full border border-border/60 bg-background/70 px-3 py-1 text-xs text-muted-foreground">
                API: {apiBaseUrl}
              </span>
              {session ? (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void logout()}
                >
                  Sign out
                </Button>
              ) : null}
              <Button variant="outline" size="sm" onClick={toggleTheme}>
                {theme === 'light' ? (
                  <MoonStar className="h-4 w-4" />
                ) : (
                  <SunMedium className="h-4 w-4" />
                )}
                Theme
              </Button>
            </div>
          </div>
        </header>
        <Outlet />
      </div>
    </div>
  );
}
