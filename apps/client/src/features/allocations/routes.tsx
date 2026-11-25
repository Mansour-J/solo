import type { RouteObject } from 'react-router';
import { AllocationsPage } from './AllocationsPage';

export const allocationRoutes: RouteObject[] = [{ path: 'allocations', element: <AllocationsPage /> }];
