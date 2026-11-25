import { ActionIcon, Avatar, Menu, Text, Tooltip, useComputedColorScheme, useMantineColorScheme } from '@mantine/core';
import { Moon, Settings, Sun, UserRound } from 'lucide-react';
import { useNavigate } from 'react-router';
import { APP_VERSION, BRAND } from '@/branding/brand';
import { useCurrentUser } from '@/features/account/api';
import { useSavePreferences } from '@/features/account/preferences';

/** Light/dark switch. The choice is saved to the owner's preferences. */
export function ColorSchemeToggle() {
  const user = useCurrentUser();
  const { setColorScheme } = useMantineColorScheme();
  const computed = useComputedColorScheme('light');
  const save = useSavePreferences();
  const next = computed === 'dark' ? 'light' : 'dark';
  return (
    <Tooltip label={`Switch to ${next} mode`}>
      <ActionIcon
        size="lg"
        aria-label={`Switch to ${next} mode`}
        onClick={() => {
          setColorScheme(next);
          save.mutate({ user, preferences: { ...user.preferences, theme: next } });
        }}
      >
        {computed === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
      </ActionIcon>
    </Tooltip>
  );
}

export function UserMenu() {
  const user = useCurrentUser();
  const initials = `${user.first_name[0] ?? ''}${user.last_name[0] ?? ''}`.toUpperCase();
  const name = `${user.first_name} ${user.last_name}`.trim();
  const navigate = useNavigate();
  return (
    <Menu width={240}>
      <Menu.Target>
        <ActionIcon size="lg" radius="xl" variant="transparent" aria-label="Your menu">
          <Avatar size={32} radius="xl" color="brand">
            {initials || <UserRound size={16} />}
          </Avatar>
        </ActionIcon>
      </Menu.Target>
      <Menu.Dropdown>
        {name && (
          <>
            <Menu.Label>
              <Text size="sm" fw={600} c="var(--mantine-color-text)" className="clamp-1">
                {name}
              </Text>
            </Menu.Label>
            <Menu.Divider />
          </>
        )}
        <Menu.Item leftSection={<UserRound size={15} />} onClick={() => navigate('/settings/profile')}>
          Profile
        </Menu.Item>
        <Menu.Item leftSection={<Settings size={15} />} onClick={() => navigate('/settings')}>
          Settings
        </Menu.Item>
        <Menu.Divider />
        <Menu.Label>
          {BRAND.name} {APP_VERSION}
        </Menu.Label>
      </Menu.Dropdown>
    </Menu>
  );
}
