import type { RouteObject } from 'react-router';
import { IncomeTaxPage } from './IncomeTaxPage';

export const incomeTaxRoutes: RouteObject[] = [{ path: 'income-tax', element: <IncomeTaxPage /> }];
