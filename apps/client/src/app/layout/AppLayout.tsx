import { AppShell, Box, Burger, Group, NavLink, ScrollArea, Stack } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { Suspense } from 'react';
import { NavLink as RouterNavLink, Outlet, useLocation } from 'react-router';
import { RecurringRunner } from '@/features/invoices/Recurring';
import { ReminderRunner } from '@/features/invoices/Reminders';
import { NoticesEmailRunner } from '@/features/settings/NoticesEmail';
import { PageLoader } from '@/app/guards';
import { FOOTER_NAV_ITEMS, NAV_ITEMS } from '@/app/nav';
import { Wordmark } from '@/components/Logo';
import { BusinessSwitcher } from './BusinessSwitcher';
import { ColorSchemeToggle, UserMenu } from './UserMenu';

function Navigation({ onNavigate }: { onNavigate: () => void }) {
  const location = useLocation();
  const isActive = (path: string) => location.pathname === path || location.pathname.startsWith(`${path}/`);
  return (
    <Stack h="100%" gap={0} justify="space-between">
      <ScrollArea type="never" style={{ flex: 1 }}>
        <Stack gap={2} p="sm">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.key}
              component={RouterNavLink}
              to={item.path}
              label={item.label}
              leftSection={<item.icon size={18} strokeWidth={1.85} />}
              active={isActive(item.path)}
              onClick={onNavigate}
              style={{ borderRadius: 8 }}
              fw={500}
            />
          ))}
        </Stack>
      </ScrollArea>
      <Stack gap={2} p="sm" style={{ borderTop: '1px solid var(--app-border)' }}>
        {FOOTER_NAV_ITEMS.map((item) => (
          <NavLink
            key={item.key}
            component={RouterNavLink}
            to={item.path}
            label={item.label}
            leftSection={<item.icon size={18} strokeWidth={1.85} />}
            active={isActive(item.path)}
            onClick={onNavigate}
            style={{ borderRadius: 8 }}
            fw={500}
          />
        ))}
      </Stack>
    </Stack>
  );
}

/** Signed-in shell: sidebar (a drawer on phones), top bar, content. */
export function AppLayout() {
  const [opened, { toggle, close }] = useDisclosure(false);
  return (
    <AppShell
      header={{ height: 'calc(3.5rem + var(--app-safe-top))' }}
      navbar={{ width: 240, breakpoint: 'sm', collapsed: { mobile: !opened } }}
      padding={0}
      transitionDuration={150}
    >
      <AppShell.Header pt="var(--app-safe-top)" style={{ borderColor: 'var(--app-border)' }}>
        <Group h="100%" px="md" justify="space-between" wrap="nowrap" gap="xs">
          <Group gap="sm" wrap="nowrap" style={{ minWidth: 0 }}>
            <Burger opened={opened} onClick={toggle} hiddenFrom="sm" size="sm" aria-label="Toggle navigation" />
            <Box visibleFrom="sm" w={208}>
              <Wordmark size={26} />
            </Box>
            <BusinessSwitcher />
          </Group>
          <Group gap={6} wrap="nowrap">
            <ColorSchemeToggle />
            <UserMenu />
          </Group>
        </Group>
      </AppShell.Header>
      <AppShell.Navbar bg="var(--app-sidebar-bg)" style={{ borderColor: 'var(--app-border)' }}>
        <Navigation onNavigate={close} />
      </AppShell.Navbar>
      <AppShell.Main bg="var(--app-surface-muted)">
        <Suspense fallback={<PageLoader />}>
          <Outlet />
          <RecurringRunner />
          <ReminderRunner />
          <NoticesEmailRunner />
        </Suspense>
      </AppShell.Main>
    </AppShell>
  );
}
