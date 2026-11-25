import { createHashRouter, Navigate, type RouteObject } from 'react-router';
import { customerRoutes } from '@/features/customers/routes';
import { DashboardPage } from '@/features/dashboard/DashboardPage';
import { expenseRoutes } from '@/features/expenses/routes';
import { gstRoutes } from '@/features/gst/routes';
import { incomeTaxRoutes } from '@/features/incometax/routes';
import { guideRoutes } from '@/features/guide/routes';
import { incomeRoutes } from '@/features/income/routes';
import { invoiceRoutes } from '@/features/invoices/routes';
import { allocationRoutes } from '@/features/allocations/routes';
import { reportRoutes } from '@/features/reports/routes';
import { insightRoutes } from '@/features/insights/routes';
import { yearEndRoutes } from '@/features/yearend/routes';
import { mileageRoutes } from '@/features/mileage/routes';
import { OnboardingPage } from '@/features/onboarding/OnboardingPage';
import { settingsRoutes } from '@/features/settings/routes';
import { RequireBusiness } from './guards';
import { AppLayout } from './layout/AppLayout';
import { NotFoundPage, RouteErrorBoundary } from './RouteError';

export const appRoutes: RouteObject[] = [
  {
    errorElement: <RouteErrorBoundary />,
    children: [
      { path: '/onboarding', element: <OnboardingPage /> },
      {
        element: <RequireBusiness />,
        children: [
          {
            path: '/',
            element: <AppLayout />,
            children: [
              { index: true, element: <Navigate to="/dashboard" replace /> },
              { path: 'dashboard', element: <DashboardPage /> },
              ...invoiceRoutes,
              ...customerRoutes,
              ...expenseRoutes,
              ...incomeRoutes,
              ...mileageRoutes,
              ...allocationRoutes,
              ...reportRoutes,
              ...gstRoutes,
              ...incomeTaxRoutes,
              ...insightRoutes,
              ...yearEndRoutes,
              ...guideRoutes,
              ...settingsRoutes,
              { path: '*', element: <NotFoundPage /> },
            ],
          },
        ],
      },
    ],
  },
];

/** Hash routing works the same in browsers and the desktop webview. */
export function createAppRouter() {
  return createHashRouter(appRoutes);
}
