import {
  BarChart3,
  BookOpen,
  CalendarCheck,
  Car,
  FileSignature,
  FileText,
  HandCoins,
  Landmark,
  LayoutDashboard,
  Lightbulb,
  PiggyBank,
  Receipt,
  ReceiptText,
  Settings,
  Users,
  type LucideIcon,
} from 'lucide-react';

export interface NavItem {
  key: string;
  label: string;
  path: string;
  icon: LucideIcon;
}

/**
 * Main navigation. A module appears here only once it works end to end (no placeholder pages),
 * in the order: Dashboard, Invoices, Quotes, Customers, Expenses, Income, Mileage, Year end, Allocations, Reports, GST, Income tax, Insights.
 */
export const NAV_ITEMS: NavItem[] = [
  { key: 'dashboard', label: 'Dashboard', path: '/dashboard', icon: LayoutDashboard },
  { key: 'invoices', label: 'Invoices', path: '/invoices', icon: FileText },
  { key: 'quotes', label: 'Quotes', path: '/quotes', icon: FileSignature },
  { key: 'customers', label: 'Customers', path: '/customers', icon: Users },
  { key: 'expenses', label: 'Expenses', path: '/expenses', icon: Receipt },
  { key: 'income', label: 'Income', path: '/income', icon: HandCoins },
  { key: 'mileage', label: 'Mileage', path: '/mileage', icon: Car },
  { key: 'year-end', label: 'Year end', path: '/year-end', icon: CalendarCheck },
  { key: 'allocations', label: 'Allocations', path: '/allocations', icon: PiggyBank },
  { key: 'reports', label: 'Reports', path: '/reports', icon: BarChart3 },
  { key: 'gst', label: 'GST', path: '/gst', icon: Landmark },
  { key: 'income-tax', label: 'Income tax', path: '/income-tax', icon: ReceiptText },
  { key: 'insights', label: 'Insights', path: '/insights', icon: Lightbulb },
];

/** Pinned to the bottom of the sidebar, below the modules: help, then settings. */
export const FOOTER_NAV_ITEMS: NavItem[] = [
  { key: 'guide', label: 'Guide', path: '/guide', icon: BookOpen },
  { key: 'settings', label: 'Settings', path: '/settings', icon: Settings },
];
