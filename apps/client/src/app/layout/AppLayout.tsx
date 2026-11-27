import { AppShell, Box, Burger, Group, NavLink, ScrollArea, Stack } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { Suspense, useState } from 'react';
import { NavLink as RouterNavLink, Outlet, useLocation } from 'react-router';
import { RecurringRunner } from '@/features/invoices/Recurring';
import { ReminderRunner } from '@/features/invoices/Reminders';
import { NoticesEmailRunner } from '@/features/settings/NoticesEmail';
import { DemoBanner } from '@/features/demo/DemoBanner';
import { PageLoader } from '@/app/guards';
import { FOOTER_NAV_ITEMS, NAV_TREE, isNavGroup, type NavItem } from '@/app/nav';
import { Wordmark } from '@/components/Logo';
import { BusinessSwitcher } from './BusinessSwitcher';
import { ColorSchemeToggle, UserMenu } from './UserMenu';

const OPEN_GROUPS_KEY = 'nav-open-groups';

function readOpenGroups(): Record<string, boolean> {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(OPEN_GROUPS_KEY) ?? '{}');
    return value && typeof value === 'object' ? (value as Record<string, boolean>) : {};
  } catch {
    return {};
  }
}

function NavItemLink({ item, active, onNavigate }: { item: NavItem; active: boolean; onNavigate: () => void }) {
  return (
    <NavLink
      component={RouterNavLink}
      to={item.path}
      label={item.label}
      leftSection={<item.icon size={18} strokeWidth={1.85} />}
      active={active}
      onClick={onNavigate}
      style={{ borderRadius: 8 }}
      fw={500}
    />
  );
}

function Navigation({ onNavigate }: { onNavigate: () => void }) {
  const location = useLocation();
  const isActive = (path: string) => location.pathname === path || location.pathname.startsWith(`${path}/`);
  // A group opens by itself while one of its screens is showing; opening or closing one by hand is remembered.
  const [openGroups, setOpenGroups] = useState(readOpenGroups);
  const toggle = (key: string, opened: boolean) => {
    const next = { ...openGroups, [key]: opened };
    setOpenGroups(next);
    try {
      window.localStorage.setItem(OPEN_GROUPS_KEY, JSON.stringify(next));
    } catch {
      // Not remembered this time; the sidebar still works.
    }
  };
  return (
    <Stack h="100%" gap={0} justify="space-between">
      <ScrollArea type="never" style={{ flex: 1 }}>
        <Stack gap={2} p="sm">
          {NAV_TREE.map((entry) => {
            if (!isNavGroup(entry))
              return <NavItemLink key={entry.key} item={entry} active={isActive(entry.path)} onNavigate={onNavigate} />;
            const holdsActive = entry.items.some((item) => isActive(item.path));
            return (
              <NavLink
                key={entry.key}
                label={entry.label}
                leftSection={<entry.icon size={18} strokeWidth={1.85} />}
                opened={openGroups[entry.key] ?? holdsActive}
                onChange={(opened) => toggle(entry.key, opened)}
                childrenOffset={14}
                style={{ borderRadius: 8 }}
                c={holdsActive ? 'var(--mantine-primary-color-filled)' : undefined}
                fw={600}
              >
                <Stack gap={2} pt={2}>
                  {entry.items.map((item) => (
                    <NavItemLink key={item.key} item={item} active={isActive(item.path)} onNavigate={onNavigate} />
                  ))}
                </Stack>
              </NavLink>
            );
          })}
        </Stack>
      </ScrollArea>
      <Stack gap={2} p="sm" style={{ borderTop: '1px solid var(--app-border)' }}>
        {FOOTER_NAV_ITEMS.map((item) => (
          <NavItemLink key={item.key} item={item} active={isActive(item.path)} onNavigate={onNavigate} />
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
        <DemoBanner />
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
