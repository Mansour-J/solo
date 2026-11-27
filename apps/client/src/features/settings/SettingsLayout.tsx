import { Box, Group, NavLink, Select, Stack, Text } from '@mantine/core';
import {
  Activity,
  Building2,
  Car,
  ClipboardCheck,
  Database,
  FileText,
  HeartPulse,
  Info,
  Scale,
  Mail,
  Palette,
  Receipt,
  RefreshCw,
  Store,
  UserRound,
  type LucideIcon,
} from 'lucide-react';
import { NavLink as RouterNavLink, Outlet, useLocation, useNavigate } from 'react-router';
import { Page } from '@/components/Page';
import { useUpdateSupport } from '@/features/updates/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { isTauri } from '@/services/platform';

interface Section {
  label: string;
  /** `desktopOnly`: the desktop app's own settings. `updates`: only when this build updates itself. */
  items: { path: string; label: string; icon: LucideIcon; desktopOnly?: boolean; updates?: boolean }[];
}

const SECTIONS: Section[] = [
  {
    label: 'Business',
    items: [
      { path: '/settings/business', label: 'Business details', icon: Building2 },
      { path: '/settings/invoicing', label: 'Invoices & payments', icon: FileText },
      { path: '/settings/expenses', label: 'Expenses', icon: Receipt },
      { path: '/settings/mileage', label: 'Mileage', icon: Car },
      { path: '/settings/health', label: 'Business health', icon: ClipboardCheck },
      { path: '/settings/activity', label: 'Activity log', icon: Activity },
      { path: '/settings/businesses', label: 'Your businesses', icon: Store },
    ],
  },
  {
    label: 'You',
    items: [
      { path: '/settings/profile', label: 'Profile', icon: UserRound },
      { path: '/settings/preferences', label: 'Preferences', icon: Palette },
      { path: '/settings/email', label: 'Email', icon: Mail },
      { path: '/settings/data', label: 'Your data', icon: Database },
      { path: '/settings/status', label: 'System status', icon: HeartPulse },
      { path: '/settings/updates', label: 'Updates', icon: RefreshCw, desktopOnly: true, updates: true },
    ],
  },
  {
    label: 'About',
    items: [
      { path: '/settings/about', label: 'About', icon: Info },
      { path: '/settings/licence', label: 'Licence', icon: Scale },
    ],
  },
];

/** The sections this app shows: updates are a desktop app setting. */
function visibleSections(updatesEnabled: boolean): Section[] {
  return SECTIONS.map((s) => ({
    ...s,
    items: s.items.filter((i) => (!i.desktopOnly || isTauri()) && (!i.updates || updatesEnabled)),
  }));
}

export function SettingsLayout() {
  const isMobile = useIsMobile();
  const updates = useUpdateSupport();
  const sections = visibleSections(updates.data?.enabled ?? false);
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <Page title="Settings" maxWidth={1100}>
      {isMobile ? (
        <Stack gap="md">
          <Select
            aria-label="Settings section"
            value={location.pathname}
            onChange={(value) => value && navigate(value)}
            data={sections.map((s) => ({ group: s.label, items: s.items.map((i) => ({ value: i.path, label: i.label })) }))}
          />
          <Outlet />
        </Stack>
      ) : (
        <Group align="flex-start" gap="xl" wrap="nowrap">
          <Stack gap="md" w={220} style={{ flexShrink: 0, position: 'sticky', top: 72 }}>
            {sections.map((section) => (
              <Box key={section.label}>
                <Text size="xs" fw={600} c="dimmed" tt="uppercase" mb={6} px="sm" style={{ letterSpacing: '0.04em' }}>
                  {section.label}
                </Text>
                {section.items.map((item) => (
                  <NavLink
                    key={item.path}
                    component={RouterNavLink}
                    to={item.path}
                    label={item.label}
                    leftSection={<item.icon size={16} />}
                    active={location.pathname === item.path}
                    style={{ borderRadius: 8 }}
                  />
                ))}
              </Box>
            ))}
          </Stack>
          <Box style={{ flex: 1, minWidth: 0 }}>
            <Outlet />
          </Box>
        </Group>
      )}
    </Page>
  );
}
