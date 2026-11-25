import type { RouteObject } from 'react-router';
import { InvoiceDetailPage, QuoteDetailPage } from './DocumentDetail';
import { InvoiceEditorPage, QuoteEditorPage } from './DocumentEditorPage';
import { InvoicesPage, QuotesPage } from './DocumentLists';
import { RecurringPage } from './Recurring';

export const invoiceRoutes: RouteObject[] = [
  { path: 'invoices', element: <InvoicesPage /> },
  { path: 'invoices/new', element: <InvoiceEditorPage /> },
  { path: 'invoices/recurring', element: <RecurringPage /> },
  { path: 'invoices/:invoiceId', element: <InvoiceDetailPage /> },
  { path: 'invoices/:invoiceId/edit', element: <InvoiceEditorPage /> },
  { path: 'quotes', element: <QuotesPage /> },
  { path: 'quotes/new', element: <QuoteEditorPage /> },
  { path: 'quotes/:quoteId', element: <QuoteDetailPage /> },
  { path: 'quotes/:quoteId/edit', element: <QuoteEditorPage /> },
];
