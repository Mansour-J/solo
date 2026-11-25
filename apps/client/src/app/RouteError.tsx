import { Button, Center, Code, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { AlertTriangle, Compass } from 'lucide-react';
import { Link, isRouteErrorResponse, useRouteError } from 'react-router';

export function NotFoundPage() {
  return (
    <Center py={80} px="md">
      <Stack align="center" gap="sm" maw={420} ta="center">
        <ThemeIcon size={48} radius="xl" variant="light" color="gray">
          <Compass size={22} />
        </ThemeIcon>
        <Title order={3}>Page not found</Title>
        <Text c="dimmed" size="sm">
          The page you’re looking for doesn’t exist or has moved.
        </Text>
        <Button component={Link} to="/dashboard" variant="default">
          Go to the dashboard
        </Button>
      </Stack>
    </Center>
  );
}

/** Last-resort error screen for errors thrown while rendering a route. */
export function RouteErrorBoundary() {
  const error = useRouteError();
  const message = isRouteErrorResponse(error)
    ? `${error.status} ${error.statusText}`
    : error instanceof Error
      ? error.message
      : 'Unknown error';
  return (
    <Center mih="100dvh" px="md">
      <Stack align="center" gap="sm" maw={480} ta="center">
        <ThemeIcon size={48} radius="xl" variant="light" color="red">
          <AlertTriangle size={22} />
        </ThemeIcon>
        <Title order={3}>Something went wrong</Title>
        <Text c="dimmed" size="sm">
          This screen hit an unexpected error. Your data is safe. Reload to try again.
        </Text>
        <Code block>{message}</Code>
        <Button onClick={() => window.location.reload()}>Reload</Button>
      </Stack>
    </Center>
  );
}
