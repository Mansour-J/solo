import {
  ActionIcon,
  Badge,
  Button,
  Card,
  createTheme,
  Drawer,
  Menu,
  Modal,
  NavLink,
  NumberInput,
  Paper,
  PasswordInput,
  Popover,
  SegmentedControl,
  Select,
  Table,
  Tabs,
  Textarea,
  TextInput,
  Tooltip,
  type MantineColorsTuple,
} from '@mantine/core';
import { BRAND } from '@/branding/brand';
import { generatePalette } from './palette';

/** Neutral slate for surfaces and text. */
const slate: MantineColorsTuple = [
  '#f6f7f9',
  '#eceef2',
  '#d7dbe3',
  '#b8bfcc',
  '#8a93a6',
  '#646e83',
  '#4a5366',
  '#353d4d',
  '#232a37',
  '#161b25',
];

export const FONT_STACK =
  '"Inter Variable", Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

export const theme = createTheme({
  primaryColor: 'brand',
  primaryShade: { light: 6, dark: 5 },
  colors: { brand: generatePalette(BRAND.primaryColor), accent: generatePalette(BRAND.secondaryColor), slate },
  autoContrast: true,
  fontFamily: FONT_STACK,
  fontFamilyMonospace: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
  fontWeights: { regular: '400', medium: '500', bold: '650' },
  headings: {
    fontFamily: FONT_STACK,
    fontWeight: '650',
    sizes: {
      h1: { fontSize: '1.625rem', lineHeight: '1.25' },
      h2: { fontSize: '1.3125rem', lineHeight: '1.3' },
      h3: { fontSize: '1.0625rem', lineHeight: '1.35' },
      h4: { fontSize: '0.9375rem', lineHeight: '1.4' },
      h5: { fontSize: '0.875rem', lineHeight: '1.4' },
      h6: { fontSize: '0.8125rem', lineHeight: '1.4' },
    },
  },
  fontSizes: { xs: '0.75rem', sm: '0.8125rem', md: '0.875rem', lg: '1rem', xl: '1.125rem' },
  lineHeights: { xs: '1.4', sm: '1.45', md: '1.5', lg: '1.55', xl: '1.6' },
  defaultRadius: 'md',
  radius: { xs: '4px', sm: '6px', md: '8px', lg: '12px', xl: '16px' },
  spacing: { xs: '0.5rem', sm: '0.75rem', md: '1rem', lg: '1.25rem', xl: '1.75rem' },
  shadows: {
    xs: '0 1px 2px rgba(16, 24, 40, 0.05)',
    sm: '0 1px 3px rgba(16, 24, 40, 0.08), 0 1px 2px rgba(16, 24, 40, 0.04)',
    md: '0 4px 12px -2px rgba(16, 24, 40, 0.08), 0 2px 4px -2px rgba(16, 24, 40, 0.04)',
    lg: '0 12px 24px -6px rgba(16, 24, 40, 0.12)',
    xl: '0 24px 48px -12px rgba(16, 24, 40, 0.18)',
  },
  cursorType: 'pointer',
  focusRing: 'auto',
  respectReducedMotion: true,
  components: {
    Button: Button.extend({ defaultProps: { size: 'sm', radius: 'md' } }),
    ActionIcon: ActionIcon.extend({ defaultProps: { variant: 'subtle', color: 'gray', radius: 'md' } }),
    Badge: Badge.extend({
      defaultProps: { variant: 'light', radius: 'sm', size: 'sm' },
      styles: { root: { textTransform: 'none', fontWeight: 550, letterSpacing: 0 } },
    }),
    Card: Card.extend({ defaultProps: { withBorder: true, radius: 'lg', padding: 'lg', shadow: 'none' } }),
    Paper: Paper.extend({ defaultProps: { radius: 'lg' } }),
    Modal: Modal.extend({ defaultProps: { radius: 'lg', centered: true, overlayProps: { backgroundOpacity: 0.45, blur: 2 } } }),
    Drawer: Drawer.extend({ defaultProps: { overlayProps: { backgroundOpacity: 0.4 } } }),
    Menu: Menu.extend({ defaultProps: { radius: 'md', shadow: 'md', position: 'bottom-end', withinPortal: true } }),
    Popover: Popover.extend({ defaultProps: { radius: 'md', shadow: 'md', withinPortal: true } }),
    Tooltip: Tooltip.extend({ defaultProps: { openDelay: 350, withArrow: true, arrowSize: 6, fz: 'xs' } }),
    TextInput: TextInput.extend({ defaultProps: { size: 'sm' } }),
    PasswordInput: PasswordInput.extend({ defaultProps: { size: 'sm' } }),
    Textarea: Textarea.extend({ defaultProps: { size: 'sm', autosize: true, minRows: 2 } }),
    NumberInput: NumberInput.extend({ defaultProps: { size: 'sm' } }),
    Select: Select.extend({ defaultProps: { size: 'sm', allowDeselect: false, comboboxProps: { shadow: 'md' } } }),
    SegmentedControl: SegmentedControl.extend({ defaultProps: { radius: 'md', size: 'xs' } }),
    Table: Table.extend({ defaultProps: { verticalSpacing: 'xs', horizontalSpacing: 'sm', highlightOnHover: true } }),
    Tabs: Tabs.extend({ defaultProps: { keepMounted: false } }),
    NavLink: NavLink.extend({ defaultProps: { variant: 'light' } }),
  },
});

/**
 * Semantic colours for financial statuses, used by every badge and chart so "overdue" always looks
 * the same. Colour is never the only signal: badges always carry a text label.
 */
export const STATUS_COLORS = {
  draft: 'gray',
  issued: 'blue',
  sent: 'blue',
  viewed: 'cyan',
  partially_paid: 'yellow',
  paid: 'teal',
  written_off: 'gray',
  overdue: 'red',
  cancelled: 'gray',
  pending: 'yellow',
  completed: 'teal',
  accepted: 'teal',
  rejected: 'red',
  expired: 'orange',
  unpaid: 'blue',
} as const;
