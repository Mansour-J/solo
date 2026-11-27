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
  Scale,
  Settings,
  ShoppingBag,
  TrendingUp,
  Users,
  Wallet,
  type LucideIcon,
} from 'lucide-react';

export interface NavItem {
  key: string;
  label: string;
  path: string;
  icon: LucideIcon;
}

/** A heading in the sidebar that opens to show its modules. */
export interface NavGroup {
  key: string;
  label: string;
  icon: LucideIcon;
  items: NavItem[];
}

/**
 * Main navigation, as the sidebar shows it: the dashboard, then the modules grouped by what they're for.
 * A module appears here only once it works end to end (no placeholder pages).
 */
export const NAV_TREE: (NavItem | NavGroup)[] = [
  { key: 'dashboard', label: 'Dashboard', path: '/dashboard', icon: LayoutDashboard },
  {
    key: 'sales',
    label: 'Sales',
    icon: Wallet,
    items: [
      { key: 'invoices', label: 'Invoices', path: '/invoices', icon: FileText },
      { key: 'quotes', label: 'Quotes', path: '/quotes', icon: FileSignature },
      { key: 'customers', label: 'Customers', path: '/customers', icon: Users },
      { key: 'income', label: 'Income', path: '/income', icon: HandCoins },
    ],
  },
  {
    key: 'spending',
    label: 'Spending',
    icon: ShoppingBag,
    items: [
      { key: 'expenses', label: 'Expenses', path: '/expenses', icon: Receipt },
      { key: 'mileage', label: 'Mileage', path: '/mileage', icon: Car },
    ],
  },
  {
    key: 'tax',
    label: 'Tax',
    icon: Scale,
    items: [
      { key: 'gst', label: 'GST', path: '/gst', icon: Landmark },
      { key: 'income-tax', label: 'Income tax', path: '/income-tax', icon: ReceiptText },
      { key: 'year-end', label: 'Year end', path: '/year-end', icon: CalendarCheck },
    ],
  },
  {
    key: 'planning',
    label: 'Planning',
    icon: TrendingUp,
    items: [
      { key: 'reports', label: 'Reports', path: '/reports', icon: BarChart3 },
      { key: 'insights', label: 'Insights', path: '/insights', icon: Lightbulb },
      { key: 'allocations', label: 'Allocations', path: '/allocations', icon: PiggyBank },
    ],
  },
];

export function isNavGroup(entry: NavItem | NavGroup): entry is NavGroup {
  return 'items' in entry;
}

/** Every module in the navigation, in sidebar order. */
export const NAV_ITEMS: NavItem[] = NAV_TREE.flatMap((entry) => (isNavGroup(entry) ? entry.items : [entry]));

/** Pinned to the bottom of the sidebar, below the modules: help, then settings. */
export const FOOTER_NAV_ITEMS: NavItem[] = [
  { key: 'guide', label: 'Guide', path: '/guide', icon: BookOpen },
  { key: 'settings', label: 'Settings', path: '/settings', icon: Settings },
];
