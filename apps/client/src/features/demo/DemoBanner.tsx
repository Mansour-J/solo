import { Alert, Button, Group, Text } from '@mantine/core';
import { Compass } from 'lucide-react';
import { useNavigate } from 'react-router';
import { useCurrentBusiness } from '@/features/businesses/api';

/** On every screen of the demo business: it's made up, and how to reset it, remove it or start for real. */
export function DemoBanner() {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  if (!business.is_demo) return null;
  return (
    <Alert color="yellow" variant="light" radius={0} icon={<Compass size={18} />} py="xs" aria-label="Demo business">
      <Group justify="space-between" gap="xs" wrap="wrap">
        <Text size="sm">
          <strong>You’re exploring the demo business.</strong> Everything in it is made up and it never sends email, so try anything.
        </Text>
        <Group gap="xs" wrap="wrap">
          <Button size="xs" onClick={() => navigate('/onboarding')}>
            Set up my business
          </Button>
          <Button size="xs" variant="default" onClick={() => navigate('/welcome?demo=reset')}>
            Reset demo
          </Button>
          <Button size="xs" variant="subtle" color="red" onClick={() => navigate('/welcome?demo=remove')}>
            Remove demo
          </Button>
        </Group>
      </Group>
    </Alert>
  );
}
