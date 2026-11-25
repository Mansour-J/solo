import { ActionIcon, Anchor, Button, Group, Paper, Stack, Text, ThemeIcon } from '@mantine/core';
import { AlertTriangle, Bell, TrendingDown, TrendingUp, X } from 'lucide-react';
import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { insightKeys, type InsightNotice, type Tone } from './api';
import { TONE_COLOURS } from './format';

const ICONS: Record<Tone, typeof Bell> = {
  positive: TrendingUp,
  negative: TrendingDown,
  warning: AlertTriangle,
  neutral: Bell,
};

/**
 * Plain-language notices with a dismiss button. `limit` shows the first few (dashboard); `dismissed`
 * offers to show dismissed ones again.
 */
export function NoticesPanel({
  notices,
  dismissed = 0,
  limit,
  title = 'Notices',
  actions,
}: {
  notices: InsightNotice[];
  dismissed?: number;
  limit?: number;
  title?: string;
  actions?: ReactNode;
}) {
  const business = useCurrentBusiness();
  const invalidate = [insightKeys.all(business.id)];
  const dismiss = useApiMutation(
    (key: string) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/insights/dismiss', { params: { path: { business_id: business.id } }, body: { key } }),
      ),
    { invalidate },
  );
  const restore = useApiMutation(
    () => unwrap(api.POST('/api/v1/businesses/{business_id}/insights/restore', { params: { path: { business_id: business.id } } })),
    { invalidate, successMessage: 'Dismissed notices are shown again' },
  );
  const shown = limit ? notices.slice(0, limit) : notices;
  return (
    <SectionCard title={title} actions={actions}>
      <Stack gap="xs">
        {shown.length === 0 ? (
          <Text size="sm" c="dimmed">
            Nothing needs your attention right now.
          </Text>
        ) : (
          shown.map((notice) => {
            const Icon = ICONS[notice.tone];
            return (
              <Paper key={notice.key} withBorder radius="md" p="sm">
                <Group gap="sm" wrap="nowrap" align="flex-start">
                  <ThemeIcon variant="light" color={TONE_COLOURS[notice.tone]} radius="xl" size="md" aria-hidden>
                    <Icon size={15} />
                  </ThemeIcon>
                  <Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
                    <Text size="sm">{notice.message}</Text>
                    {notice.link && (
                      <Anchor component={Link} to={notice.link} size="xs">
                        Take a look
                      </Anchor>
                    )}
                  </Stack>
                  <ActionIcon
                    variant="subtle"
                    color="gray"
                    aria-label={`Dismiss: ${notice.message}`}
                    loading={dismiss.isPending && dismiss.variables === notice.key}
                    onClick={() => dismiss.mutate(notice.key)}
                  >
                    <X size={16} />
                  </ActionIcon>
                </Group>
              </Paper>
            );
          })
        )}
        {limit && notices.length > limit && (
          <Anchor component={Link} to="/insights" size="sm">
            {notices.length - limit} more on the Insights page
          </Anchor>
        )}
        {dismissed > 0 && (
          <Group>
            <Button variant="subtle" size="xs" color="gray" loading={restore.isPending} onClick={() => restore.mutate(undefined)}>
              Show {dismissed} dismissed {dismissed === 1 ? 'notice' : 'notices'} again
            </Button>
          </Group>
        )}
      </Stack>
    </SectionCard>
  );
}
