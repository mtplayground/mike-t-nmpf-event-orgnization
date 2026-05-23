import { RouterProvider } from 'react-router-dom';

import { AppProviders } from '@/app/providers';
import { router } from '@/app/router';
import { ErrorBoundary } from '@/components/error-boundary';

export default function App() {
  return (
    <ErrorBoundary>
      <AppProviders>
        <RouterProvider router={router} />
      </AppProviders>
    </ErrorBoundary>
  );
}
