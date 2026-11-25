import { Anchor } from '@mantine/core';
import { Link } from 'react-router';
import { useInsights } from './api';
import { NoticesPanel } from './NoticesPanel';

/** The first three notices on the dashboard; hidden while loading or when there are none. */
export function DashboardNotices() {
  const insights = useInsights();
  if (!insights.data || insights.data.notices.length === 0) return null;
  return (
    <NoticesPanel
      notices={insights.data.notices}
      limit={3}
      title="Worth knowing"
      actions={
        <Anchor component={Link} to="/insights" size="sm">
          All insights
        </Anchor>
      }
    />
  );
}
