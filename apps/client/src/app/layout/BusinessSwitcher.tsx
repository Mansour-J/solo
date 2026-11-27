import { Avatar, Badge, Group, Menu, Text, UnstyledButton } from '@mantine/core';
import { Check, ChevronsUpDown, Plus, Settings } from 'lucide-react';
import { useNavigate } from 'react-router';
import { useBusinesses, useCurrentBusiness } from '@/features/businesses/api';
import { useSessionStore } from '@/stores/session';

function initials(name: string) {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('');
}

/** Shows the current business and switches between the user's businesses. */
export function BusinessSwitcher() {
  const business = useCurrentBusiness();
  const { data: businesses = [] } = useBusinesses();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const navigate = useNavigate();

  return (
    <Menu position="bottom-start" width={280}>
      <Menu.Target>
        <UnstyledButton
          aria-label={`Current business: ${business.name}. Switch business`}
          px="xs"
          py={6}
          style={{ borderRadius: 8, maxWidth: 320 }}
        >
          <Group gap="xs" wrap="nowrap">
            <Avatar size={28} radius="md" color="brand" variant="light">
              {initials(business.name)}
            </Avatar>
            <Text fw={600} size="sm" className="clamp-1">
              {business.name}
            </Text>
            {business.is_demo && (
              <Badge size="xs" color="yellow" variant="light" style={{ flexShrink: 0 }}>
                Demo
              </Badge>
            )}
            <ChevronsUpDown size={14} style={{ flexShrink: 0, opacity: 0.6 }} />
          </Group>
        </UnstyledButton>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>Your businesses</Menu.Label>
        {businesses.map((b) => (
          <Menu.Item
            key={b.id}
            leftSection={
              <Avatar size={22} radius="sm" color="brand" variant="light">
                {initials(b.name)}
              </Avatar>
            }
            rightSection={b.id === business.id ? <Check size={14} /> : null}
            onClick={() => {
              selectBusiness(b.id);
              navigate('/dashboard');
            }}
          >
            <Group gap={6} wrap="nowrap">
              <Text size="sm" className="clamp-1">
                {b.name}
              </Text>
              {b.is_demo && (
                <Badge size="xs" color="yellow" variant="light" style={{ flexShrink: 0 }}>
                  Demo
                </Badge>
              )}
            </Group>
          </Menu.Item>
        ))}
        <Menu.Divider />
        <Menu.Item leftSection={<Settings size={15} />} onClick={() => navigate('/settings/business')}>
          Business settings
        </Menu.Item>
        <Menu.Item leftSection={<Plus size={15} />} onClick={() => navigate('/onboarding')}>
          Add a business
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
}
