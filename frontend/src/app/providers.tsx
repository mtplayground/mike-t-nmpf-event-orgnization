import { type PropsWithChildren, useEffect, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';

import { appConfig } from '@/lib/config';
import { createQueryClient } from '@/lib/query-client';
import { useAppStore } from '@/stores/app-store';
import { useAuthStore } from '@/stores/auth-store';

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(createQueryClient);
  const theme = useAppStore((state) => state.theme);
  const setApiBaseUrl = useAppStore((state) => state.setApiBaseUrl);
  const hydrate = useAuthStore((state) => state.hydrate);
  const initialized = useAuthStore((state) => state.initialized);
  const refreshSession = useAuthStore((state) => state.refreshSession);
  const session = useAuthStore((state) => state.session);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark');
  }, [theme]);

  useEffect(() => {
    setApiBaseUrl(appConfig.apiBaseUrl);
  }, [setApiBaseUrl]);

  useEffect(() => {
    void hydrate();
  }, [hydrate]);

  useEffect(() => {
    if (!initialized || !session) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void refreshSession();
    }, 30_000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [initialized, refreshSession, session]);

  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}
