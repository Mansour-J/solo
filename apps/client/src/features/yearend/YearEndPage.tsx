import { Tabs } from '@mantine/core';
import { useSearchParams } from 'react-router';
import { Page } from '@/components/Page';
import { AssetsPanel } from './AssetsPanel';
import { HomeOfficePanel } from './HomeOfficePanel';
import { MovementsPanel } from './MovementsPanel';
import { OpeningPanel } from './OpeningPanel';
import { YEAR_END_TABS } from './forms';

type Tab = (typeof YEAR_END_TABS)[number];

/** What an accountant needs beyond income and expenses: money in and out, assets, opening balances. */
export function YearEndPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const requested = searchParams.get('tab');
  const tab: Tab = YEAR_END_TABS.includes(requested as Tab) ? (requested as Tab) : 'money';
  return (
    <Page
      title="Year end"
      description="Drawings, tax payments, assets, home office and opening balances: what your balance sheet and annual accounts need beyond income and expenses."
    >
      <Tabs
        value={tab}
        onChange={(value) =>
          setSearchParams(
            (params) => {
              const next = new URLSearchParams(params);
              next.set('tab', value ?? 'money');
              return next;
            },
            { replace: true },
          )
        }
        keepMounted={false}
      >
        <Tabs.List mb="lg">
          <Tabs.Tab value="money">Money in and out</Tabs.Tab>
          <Tabs.Tab value="assets">Assets</Tabs.Tab>
          <Tabs.Tab value="home-office">Home office</Tabs.Tab>
          <Tabs.Tab value="opening">Opening balances</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="money">
          <MovementsPanel />
        </Tabs.Panel>
        <Tabs.Panel value="assets">
          <AssetsPanel />
        </Tabs.Panel>
        <Tabs.Panel value="home-office">
          <HomeOfficePanel />
        </Tabs.Panel>
        <Tabs.Panel value="opening">
          <OpeningPanel />
        </Tabs.Panel>
      </Tabs>
    </Page>
  );
}
