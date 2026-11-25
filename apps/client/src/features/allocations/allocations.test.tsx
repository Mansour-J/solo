import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Allocation, Balances, Destination, PlanView, Rule } from './api';
import { AllocationsPage } from './AllocationsPage';
import { destinationSchema, emptyDestinationValues, toDestinationInput } from './destinationForm';
import { formatPercentage, ruleAmountText, ruleDetails } from './format';
import { ReviewModal } from './ReviewModal';
import { emptyRuleValues, fromRule, ruleSchema, toRuleInput, type RuleFormValues } from './ruleForm';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    today: '2026-11-20',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => {
  vi.restoreAllMocks();
});

function destination(id: string, name: string, extra: Partial<Destination> = {}): Destination {
  return {
    id,
    name,
    kind: 'custom',
    position: 1,
    target_amount: null,
    minimum_balance: null,
    notes: '',
    archived: false,
    balance: 0,
    rule_count: 0,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...extra,
  };
}

const rule: Rule = {
  id: 'rule1',
  destination_id: 'd1',
  destination_name: 'Income tax',
  priority: 1,
  method: 'percentage',
  percentage: '20.00',
  basis: 'received',
  fixed_amount: null,
  source: 'invoice_payment',
  income_type: null,
  customer_id: null,
  customer_name: null,
  active_from: null,
  active_to: null,
  max_amount: 50_000,
  cap_amount: 100_000,
  cap_period: 'month',
  cap_period_days: null,
  cap_anchor: null,
  stop_at_target: true,
  notes: '',
  archived: false,
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

describe('rule form', () => {
  const valid = (values: Partial<RuleFormValues>) => ruleSchema.safeParse({ ...emptyRuleValues('d1', '2026-11-20'), ...values });
  const fields = (values: Partial<RuleFormValues>) => {
    const result = valid(values);
    return result.success ? [] : result.error.issues.map((i) => i.path.join('.'));
  };

  it('mirrors the API’s checks with the API’s field names', () => {
    expect(fields({ percentage: '20' })).toEqual([]);
    expect(fields({ percentage: '' })).toEqual(['percentage']);
    expect(fields({ percentage: '120' })).toEqual(['percentage']);
    expect(fields({ percentage: '12.555' })).toEqual(['percentage']);
    expect(fields({ method: 'fixed', fixed_amount: null })).toEqual(['fixed_amount']);
    expect(fields({ destination_id: null, percentage: '5' })).toEqual(['destination_id']);
    expect(fields({ percentage: '5', active_from: '2026-06-01', active_to: '2026-05-01' })).toEqual(['active_to']);
    expect(fields({ percentage: '5', capped: true, cap_amount: null })).toEqual(['cap_amount']);
    expect(fields({ percentage: '5', capped: true, cap_amount: 100, cap_period: 'custom', cap_period_days: 0 })).toEqual([
      'cap_period_days',
    ]);
    expect(fields({ percentage: '5', capped: true, cap_amount: 100, cap_period: 'fortnight', cap_anchor: null })).toEqual(['cap_anchor']);
  });

  it('sends only the fields for the chosen method, source and cap', () => {
    const percentage = toRuleInput({
      ...emptyRuleValues('d1', '2026-11-20'),
      percentage: ' 12.5 ',
      fixed_amount: 999,
      income_type: 'sales',
    });
    expect(percentage).toMatchObject({ method: 'percentage', percentage: '12.5', fixed_amount: null, income_type: null, cap_amount: null });
    const fixed = toRuleInput({
      ...emptyRuleValues('d1', '2026-11-20'),
      method: 'fixed',
      fixed_amount: 5_000,
      source: 'income',
      income_type: 'other',
      capped: true,
      cap_amount: 20_000,
      cap_period: 'month',
    });
    expect(fixed).toMatchObject({ percentage: null, fixed_amount: 5_000, income_type: 'other', cap_period: 'month', cap_anchor: null });
    const fortnight = toRuleInput({
      ...emptyRuleValues('d1', '2026-11-20'),
      percentage: '5',
      capped: true,
      cap_amount: 1,
      cap_period: 'fortnight',
    });
    expect(fortnight).toMatchObject({ cap_anchor: '2026-11-20', cap_period_days: null });
  });

  it('round-trips a saved rule', () => {
    const values = fromRule(rule, '2026-11-20');
    expect(values).toMatchObject({ percentage: '20', capped: true, cap_period: 'month', stop_at_target: true });
    expect(toRuleInput(values)).toMatchObject({ percentage: '20', max_amount: 50_000, cap_amount: 100_000, source: 'invoice_payment' });
  });

  it('describes rules in words', () => {
    expect(formatPercentage('20.00')).toBe('20');
    expect(formatPercentage('12.50')).toBe('12.5');
    expect(ruleAmountText(rule, 'NZD')).toBe('20% of the amount received');
    expect(ruleDetails(rule, 'NZD')).toEqual([
      'Invoice payments only',
      'At most $500.00 per receipt',
      'At most $1,000.00 a month',
      'Stops at the target',
    ]);
  });
});

describe('destination form', () => {
  it('checks names and optional amounts', () => {
    const issues = (values: object) => {
      const result = destinationSchema.safeParse({ ...emptyDestinationValues(), ...values });
      return result.success ? [] : result.error.issues.map((i) => i.path.join('.'));
    };
    expect(issues({ name: 'Holidays' })).toEqual([]);
    expect(issues({ name: '  ' })).toEqual(['name']);
    expect(issues({ name: 'Holidays', target_amount: 0 })).toEqual(['target_amount']);
    expect(toDestinationInput({ ...emptyDestinationValues(), name: ' Holidays ', target_amount: 10_000 })).toEqual({
      name: 'Holidays',
      kind: 'custom',
      target_amount: 10_000,
      minimum_balance: null,
      notes: '',
    });
  });
});

const plan: PlanView = {
  received_amount: 100_000,
  allocated: 25_000,
  remaining: 75_000,
  currency: 'NZD',
  lines: [
    {
      rule_id: 'rule1',
      destination_id: 'd1',
      destination_name: 'Income tax',
      requested: 20_000,
      amount: 20_000,
      limited_by: null,
      balance: 0,
      balance_after: 20_000,
      target_amount: null,
    },
    {
      rule_id: 'rule2',
      destination_id: 'd2',
      destination_name: 'Savings',
      requested: 10_000,
      amount: 5_000,
      limited_by: 'cap',
      balance: 0,
      balance_after: 5_000,
      target_amount: null,
    },
  ],
};

const pending: Allocation = {
  id: 'a1',
  source_type: 'invoice_payment',
  invoice_payment_id: 'p1',
  income_id: null,
  invoice_id: 'i1',
  source_label: 'Invoice INV-0007',
  customer_id: 'c1',
  customer_name: 'Kākā Studio',
  income_type: null,
  received_date: '2026-11-18',
  received_amount: 100_000,
  currency: 'NZD',
  status: 'pending',
  allocated: 0,
  confirmed_at: null,
  closed_at: null,
  close_reason: null,
  replaces_id: null,
  created_at: '2026-11-18T00:00:00Z',
  entries: [],
  plan,
};

interface Sent {
  url: string;
  body: unknown;
}

function mockApi({ allocation = pending, balances }: { allocation?: Allocation; balances?: Balances } = {}) {
  const sent: Sent[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    const url = new URL(request.url).pathname;
    if (request.method === 'GET') {
      if (url.endsWith('/allocations/destinations')) {
        return Response.json([destination('d1', 'Income tax'), destination('d2', 'Savings'), destination('d3', 'KiwiSaver')]);
      }
      if (url.endsWith('/allocations/balances') && balances) return Response.json(balances);
      if (url.endsWith(`/allocations/${allocation.id}`)) return Response.json(allocation);
      return Response.json({ items: [], page: 1, per_page: 20, total: 0 });
    }
    const body = JSON.parse(await request.clone().text()) as { lines?: { amount: number }[] };
    sent.push({ url, body });
    if (url.endsWith('/calculate')) {
      const allocated = (body.lines ?? []).reduce((sum, l) => sum + l.amount, 0);
      return Response.json({ ...plan, allocated, remaining: 100_000 - allocated, lines: [] });
    }
    return Response.json({ ...allocation, status: 'confirmed' });
  });
  return sent;
}

describe('review modal', () => {
  it('shows the plan, lets amounts change and confirms what the user chose', async () => {
    const sent = mockApi();
    const onClose = vi.fn();
    renderWithProviders(<ReviewModal allocationId="a1" onClose={onClose} />);
    expect(await screen.findByText('Invoice INV-0007')).toBeInTheDocument();
    expect(screen.getByText('Limited by the cap for the period')).toBeInTheDocument();
    expect(screen.getByText(/never|doesn’t move money/)).toBeInTheDocument();

    const tax = screen.getByLabelText('Amount for Income tax');
    await userEvent.clear(tax);
    await userEvent.type(tax, '300');
    expect(await screen.findByText('Proposed $200.00')).toBeInTheDocument();
    // Totals come from the API's calculation.
    await waitFor(() => expect(sent.some((s) => s.url.endsWith('/calculate'))).toBe(true));
    expect(await screen.findByText('$650.00')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Confirm allocation' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const confirm = sent.find((s) => s.url.endsWith('/confirm'));
    expect(confirm?.body).toEqual({
      lines: [
        { destination_id: 'd1', amount: 30_000, rule_id: 'rule1' },
        { destination_id: 'd2', amount: 5_000, rule_id: 'rule2' },
      ],
    });
  });
});

describe('allocations page', () => {
  it('invites the user to add a rule before anything is planned', async () => {
    mockApi({ balances: { currency: 'NZD', destinations: [], total: 0, pending_count: 0, has_rules: false } });
    renderWithProviders(<AllocationsPage />, { path: '/allocations' });
    expect(await screen.findByText('Plan where your money goes')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add your first rule' })).toBeInTheDocument();
  });

  it('shows balances with a table view', async () => {
    mockApi({
      balances: {
        currency: 'NZD',
        destinations: [
          {
            id: 'd1',
            name: 'Income tax',
            kind: 'income_tax',
            archived: false,
            balance: 30_000,
            target_amount: null,
            target_progress: null,
            minimum_balance: null,
            below_minimum: false,
          },
          {
            id: 'd2',
            name: 'Emergency fund',
            kind: 'emergency_fund',
            archived: false,
            balance: 10_000,
            target_amount: 20_000,
            target_progress: 50,
            minimum_balance: 15_000,
            below_minimum: true,
          },
        ],
        total: 40_000,
        pending_count: 0,
        has_rules: true,
      },
    });
    renderWithProviders(<AllocationsPage />, { path: '/allocations' });
    expect(await screen.findByText('$400.00 set aside in total')).toBeInTheDocument();
    expect(screen.getByText('Below minimum')).toBeInTheDocument();
    expect(screen.getByText(/50% of \$200\.00 target/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('radio', { name: 'Table' }));
    expect(screen.getByRole('columnheader', { name: 'Share' })).toBeInTheDocument();
    expect(screen.getByText('Nothing to review')).toBeInTheDocument();
  });
});
