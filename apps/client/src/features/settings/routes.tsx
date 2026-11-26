import { Navigate, type RouteObject } from 'react-router';
import { ExpenseCategoriesSettings } from '@/features/expenses/ExpenseCategoriesSettings';
import { BusinessHealthPage } from '@/features/health/BusinessHealthPage';
import { MileageSettings } from '@/features/mileage/MileageSettings';
import { UpdatesSettings } from '@/features/updates/UpdatesSettings';
import { DataSettings, PreferencesSettings, ProfileSettings } from './AccountSettings';
import { ActivitySettings, BusinessesSettings, InvoicingSettings, KeyedBusinessDetails } from './BusinessSettings';
import { EmailSettingsPage } from './EmailSettings';
import { SettingsLayout } from './SettingsLayout';
import { SystemStatusPage } from './SystemStatus';

export const settingsRoutes: RouteObject[] = [
  {
    path: 'settings',
    element: <SettingsLayout />,
    children: [
      { index: true, element: <Navigate to="business" replace /> },
      { path: 'business', element: <KeyedBusinessDetails /> },
      { path: 'invoicing', element: <InvoicingSettings /> },
      { path: 'expenses', element: <ExpenseCategoriesSettings /> },
      { path: 'mileage', element: <MileageSettings /> },
      { path: 'health', element: <BusinessHealthPage /> },
      { path: 'activity', element: <ActivitySettings /> },
      { path: 'businesses', element: <BusinessesSettings /> },
      { path: 'profile', element: <ProfileSettings /> },
      { path: 'preferences', element: <PreferencesSettings /> },
      { path: 'email', element: <EmailSettingsPage /> },
      { path: 'data', element: <DataSettings /> },
      { path: 'status', element: <SystemStatusPage /> },
      { path: 'updates', element: <UpdatesSettings /> },
    ],
  },
];
