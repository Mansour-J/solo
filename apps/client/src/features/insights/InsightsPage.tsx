import { SimpleGrid, Stack, Text, Title } from '@mantine/core';
import { Page } from '@/components/Page';
import { ErrorState, ListSkeleton } from '@/components/States';
import { formatDate } from '@/lib/format';
import { useInsights, type Insights } from './api';
import { InsightCard } from './InsightCard';
import { NoticesPanel } from './NoticesPanel';

const DESCRIPTION =
  'What your figures mean, worked out from your records with simple, explained rules. Estimates are marked, and nothing is shown without enough data behind it.';

function Sections({ insights }: { insights: Insights }) {
  return (
    <Stack gap="xl">
      <NoticesPanel notices={insights.notices} dismissed={insights.dismissed_count} />
      {insights.sections.map((section) => (
        <Stack key={section.key} gap="sm">
          <div>
            <Title order={2} size="h4">
              {section.title}
            </Title>
            <Text size="sm" c="dimmed">
              {section.description}
            </Text>
          </div>
          {section.items.length === 0 ? (
            <Text size="sm" c="dimmed">
              {section.empty_reason}
            </Text>
          ) : (
            <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="md">
              {section.items.map((item) => (
                <InsightCard key={item.key} insight={item} currency={insights.currency} />
              ))}
            </SimpleGrid>
          )}
        </Stack>
      ))}
      <Text size="xs" c="dimmed">
        Based on your records on the {insights.basis} basis, as at {formatDate(insights.generated_on)}. Insights aren’t financial or tax
        advice.
      </Text>
    </Stack>
  );
}

/** Insights in four sections, with dismissible notices first. */
export function InsightsPage() {
  const insights = useInsights();
  return (
    <Page title="Insights" description={DESCRIPTION}>
      {insights.isPending ? (
        <ListSkeleton rows={6} height={120} />
      ) : insights.isError ? (
        <ErrorState error={insights.error} onRetry={() => void insights.refetch()} />
      ) : (
        <Sections insights={insights.data} />
      )}
    </Page>
  );
}
