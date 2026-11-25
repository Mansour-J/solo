import type { RouteObject } from 'react-router';
import { ExpensesPage } from './ExpensesPage';

export const expenseRoutes: RouteObject[] = [{ path: 'expenses', element: <ExpensesPage /> }];
