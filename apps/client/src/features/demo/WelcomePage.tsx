import { Alert, Anchor, Box, Button, Container, Group, Paper, Progress, SimpleGrid, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { Briefcase, Compass } from 'lucide-react';
import type { ReactNode } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { BRAND } from '@/branding/brand';
import { Wordmark } from '@/components/Logo';
import { useBusinesses } from '@/features/businesses/api';
import { errorMessage } from '@/services/api/errors';
import { useSessionStore } from '@/stores/session';
import { rememberChoice, useDemo } from './api';

function Choice(props: { icon: ReactNode; title: string; children: ReactNode; action: ReactNode }) {
  return (
    <Paper withBorder p={{ base: 'md', sm: 'xl' }} radius="lg" bg="var(--app-surface-raised)">
      <Stack gap="md" h="100%" justify="space-between">
        <Stack gap="sm">
          <ThemeIcon size={44} radius="md" variant="light">
            {props.icon}
          </ThemeIcon>
          <Title order={2} size="h3">
            {props.title}
          </Title>
          <Text size="sm" c="dimmed">
            {props.children}
          </Text>
        </Stack>
        {props.action}
      </Stack>
    </Paper>
  );
}

/**
 * The first-run screen: explore a demo business of made-up records, or set up your own (SL-710). Also
 * resets (`?demo=reset`) and removes (`?demo=remove`) the demo, away from the business's own screens.
 */
export function WelcomePage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const mode = params.get('demo');
  const { data: businesses = [] } = useBusinesses();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const demo = useDemo();
  const existing = businesses.find((b) => b.is_demo);
  const hasOwn = businesses.some((b) => !b.is_demo);

  async function explore() {
    if (await demo.build()) navigate('/dashboard', { replace: true });
  }

  async function remove() {
    navigate((await demo.remove()) ? '/dashboard' : '/welcome', { replace: true });
  }

  function setUp() {
    rememberChoice('setup');
    navigate('/onboarding');
  }

  let content: ReactNode;
  if (demo.progress !== null) {
    content = (
      <Paper withBorder p={{ base: 'md', sm: 'xl' }} radius="lg" bg="var(--app-surface-raised)">
        <Stack gap="sm">
          <Text fw={600}>Filling the demo business with invoices, expenses, GST returns and trips…</Text>
          <Progress value={Math.round(demo.progress * 100)} aria-label="Demo progress" animated />
          <Text size="sm" c="dimmed">
            {Math.round(demo.progress * 100)}%. This takes a few seconds.
          </Text>
        </Stack>
      </Paper>
    );
  } else if (mode === 'reset' || mode === 'remove') {
    const reset = mode === 'reset';
    content = (
      <Paper withBorder p={{ base: 'md', sm: 'xl' }} radius="lg" bg="var(--app-surface-raised)">
        <Stack gap="md">
          <Text>
            {reset
              ? 'The demo business goes back to how it started: anything you’ve added or changed in it is removed.'
              : 'The demo business and everything in it are deleted. You can explore it again later from Add a business.'}{' '}
            Your own businesses aren’t touched.
          </Text>
          {demo.error !== null && (
            <Alert color="red" title="That didn’t work">
              {errorMessage(demo.error)}
            </Alert>
          )}
          <Group>
            <Button color={reset ? undefined : 'red'} onClick={() => void (reset ? explore() : remove())}>
              {reset ? 'Reset the demo' : 'Remove the demo'}
            </Button>
            <Button variant="default" onClick={() => navigate('/dashboard')}>
              Cancel
            </Button>
          </Group>
        </Stack>
      </Paper>
    );
  } else {
    content = (
      <Stack gap="md">
        {demo.error !== null && (
          <Alert color="red" title="The demo couldn’t be set up">
            {errorMessage(demo.error)} Try again, or set up your business instead.
          </Alert>
        )}
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
          <Choice
            icon={<Compass size={24} />}
            title="Explore with demo data"
            action={
              existing ? (
                <Button
                  onClick={() => {
                    selectBusiness(existing.id);
                    navigate('/dashboard');
                  }}
                >
                  Open the demo
                </Button>
              ) : (
                <Button onClick={() => void explore()}>{demo.error !== null ? 'Try again' : 'Explore the demo'}</Button>
              )
            }
          >
            A made-up contractor’s books: a year and a half of invoices, expenses, GST returns, mileage, assets and reports. Try anything.
            Nothing in it is real, it never sends email, and you can reset it whenever you like.
          </Choice>
          <Choice
            icon={<Briefcase size={24} />}
            title="Set up my business"
            action={
              <Button variant="default" onClick={setUp}>
                Set up my business
              </Button>
            }
          >
            Start with empty books and enter your own business: its name, GST and financial year. It takes a few minutes, and anything
            optional can be finished later in Settings.
          </Choice>
        </SimpleGrid>
        <Text size="sm" c="dimmed">
          You can do both. The demo is a separate business, labelled on every screen, so it never mixes with your own records, and you can
          remove it whenever you like.
        </Text>
      </Stack>
    );
  }

  const title =
    demo.progress !== null
      ? 'Setting up the demo'
      : mode === 'reset'
        ? 'Reset the demo?'
        : mode === 'remove'
          ? 'Remove the demo?'
          : `Welcome to ${BRAND.name}`;

  return (
    <Box mih="100dvh" bg="var(--app-surface-muted)">
      <Group justify="space-between" px={{ base: 'md', sm: 'xl' }} py="md">
        <Wordmark />
        {(hasOwn || existing) && !mode && demo.progress === null && (
          <Anchor size="sm" onClick={() => navigate('/')}>
            Cancel
          </Anchor>
        )}
      </Group>
      <Container size={820} pb="xl">
        <Stack gap="lg">
          <Stack gap={4}>
            <Title order={1}>{title}</Title>
            {!mode && demo.progress === null && (
              <Text c="dimmed">Bookkeeping, GST and tax for New Zealand sole traders. How would you like to start?</Text>
            )}
          </Stack>
          {content}
        </Stack>
      </Container>
    </Box>
  );
}
