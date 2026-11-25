import type { RouteObject } from 'react-router';
import { CustomerDetailPage } from './CustomerDetailPage';
import { CustomersPage } from './CustomersPage';

export const customerRoutes: RouteObject[] = [
  { path: 'customers', element: <CustomersPage /> },
  { path: 'customers/:customerId', element: <CustomerDetailPage /> },
];
