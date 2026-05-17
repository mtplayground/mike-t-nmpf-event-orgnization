import { type PropsWithChildren, useEffect, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';

import { appConfig } from '@/lib/config';
import { createQueryClient } from '@/lib/query-client';
import { useAppStore } from '@/stores/app-store';

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(createQueryClient);
  const theme = useAppStore((state) => state.theme);
  const setApiBaseUrl = useAppStore((state) => state.setApiBaseUrl);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark');
  }, [theme]);

  useEffect(() => {
    setApiBaseUrl(appConfig.apiBaseUrl);
  }, [setApiBaseUrl]);

  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}
