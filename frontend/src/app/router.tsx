import { createBrowserRouter } from 'react-router-dom';

import { RootLayout } from '@/components/root-layout';
import { AttendeePage } from '@/pages/attendee-page';
import { AuthPage } from '@/pages/auth-page';
import { HomePage } from '@/pages/home-page';
import { HostPage } from '@/pages/host-page';
import { NotFoundPage } from '@/pages/not-found-page';

export const router = createBrowserRouter([
  {
    path: '/',
    element: <RootLayout />,
    children: [
      { index: true, element: <HomePage /> },
      { path: 'auth', element: <AuthPage /> },
      { path: 'host', element: <HostPage /> },
      { path: 'attendee', element: <AttendeePage /> },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]);
