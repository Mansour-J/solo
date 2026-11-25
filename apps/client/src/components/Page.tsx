import { Box, Group, Stack, Text, Title, type BoxProps } from '@mantine/core';
import type { ReactNode } from 'react';

export interface PageProps extends BoxProps {
  title: ReactNode;
  description?: ReactNode;
  /** Primary actions: right-aligned on desktop, wrapped under the title on small screens. */
  actions?: ReactNode;
  /** Optional element above the title (back link, breadcrumbs). */
  eyebrow?: ReactNode;
  children: ReactNode;
  /** Narrower content for forms and settings. */
  maxWidth?: number | string;
}

/** Standard page: consistent header, spacing and maximum width. */
export function Page({ title, description, actions, eyebrow, children, maxWidth, ...boxProps }: PageProps) {
  return (
    <Box px={{ base: 'md', sm: 'xl' }} py={{ base: 'md', sm: 'lg' }} pb="xl" mx="auto" w="100%" maw={maxWidth ?? 1280} {...boxProps}>
      <Stack gap="lg">
        <Group justify="space-between" align="flex-end" gap="md">
          <Stack gap={4} style={{ minWidth: 0, flex: '1 1 320px' }}>
            {eyebrow}
            <Title order={1} fz={{ base: 'h2', sm: 'h1' }}>
              {title}
            </Title>
            {description && (
              <Text c="dimmed" size="sm" maw={720}>
                {description}
              </Text>
            )}
          </Stack>
          {actions && (
            <Group gap="xs" wrap="wrap" style={{ flex: '0 1 auto', minWidth: 0, maxWidth: '100%' }}>
              {actions}
            </Group>
          )}
        </Group>
        {children}
      </Stack>
    </Box>
  );
}
