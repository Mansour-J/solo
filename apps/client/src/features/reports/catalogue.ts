// The reports and the filters each one uses.
import type { ReportKind } from './api';

export interface ReportInfo {
  kind: ReportKind;
  label: string;
  description: string;
  /** Uses a date range. */
  period: boolean;
  /** Reads the journal on a cash or accrual basis. */
  basis: boolean;
  customer?: 'optional' | 'required';
  category?: boolean;
  vehicle?: boolean;
  status?: boolean;
}

export const REPORTS: ReportInfo[] = [
  {
    kind: 'profit_and_loss',
    label: 'Profit and loss',
    description: 'Income and expenses by account, non-deductible adjustments, gross and net profit, compared with the previous period.',
    period: true,
    basis: true,
  },
  {
    kind: 'balance_sheet',
    label: 'Balance sheet',
    description: 'What the business owns and owes at the end of the period, and the owner’s equity, beside the year before.',
    period: true,
    basis: true,
  },
  {
    kind: 'equity_changes',
    label: 'Statement of changes in equity',
    description: 'Opening equity, profit, drawings, funds introduced and tax paid, to closing equity.',
    period: true,
    basis: true,
  },
  {
    kind: 'depreciation_schedule',
    label: 'Depreciation schedule',
    description: 'Each asset’s cost, opening value, depreciation and closing value for the financial year.',
    period: true,
    basis: false,
  },
  {
    kind: 'income_summary',
    label: 'Income summary',
    description: 'Income by month, customer, source and financial year, and paid versus outstanding invoices.',
    period: true,
    basis: true,
    customer: 'optional',
  },
  {
    kind: 'expense_summary',
    label: 'Expense summary',
    description: 'Expenses by category, month and supplier, with GST claimed and the private share.',
    period: true,
    basis: true,
    category: true,
  },
  {
    kind: 'statement_of_account',
    label: 'Statement of account',
    description: 'A customer’s invoices, payments and balance for the period, ready to send as a PDF.',
    period: true,
    basis: false,
    customer: 'required',
  },
  {
    kind: 'receivables',
    label: 'Accounts receivable',
    description: 'What customers owe today, by how long it’s been overdue.',
    period: false,
    basis: false,
    customer: 'optional',
  },
  {
    kind: 'financial_statement',
    label: 'Financial statement',
    description: 'Monthly income, expenses and net position with a summary, for a bank or broker.',
    period: true,
    basis: true,
  },
  {
    kind: 'gst',
    label: 'GST summary',
    description: 'GST collected and claimable, with every record behind the figures. Informational, not a filed return.',
    period: true,
    basis: true,
  },
  {
    kind: 'mileage',
    label: 'Mileage',
    description: 'Trips, kilometres and claims by month and vehicle.',
    period: true,
    basis: false,
    vehicle: true,
  },
  {
    kind: 'allocations',
    label: 'Allocations',
    description: 'Balances, what was set aside in the period, the rules and the history.',
    period: true,
    basis: false,
    status: true,
  },
];

export function reportInfo(kind: string | null): ReportInfo {
  return REPORTS.find((r) => r.kind === kind) ?? REPORTS[0]!;
}
