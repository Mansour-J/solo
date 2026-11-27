// The demo business: a made-up contractor, "Fernbird Consulting (demo)", with the last financial year
// and this one so far: monthly invoices to a recruitment agency with tax withheld, direct jobs, a quote,
// GST every two months on the payments basis (returns paid), expenses, assets, home office, mileage with
// IRD's two tiers, drawings and allocation rules. Every name and figure is invented. Dates follow the
// business's today, so the demo always looks current.
//
// Plain TypeScript with no imports, so `scripts/seed-demo.mjs` can run it with Node as well as the app.

/** Sends one API request (path under `/api/v1`) and returns its JSON. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- replies are read loosely, as the API returns them
export type DemoCall = (method: string, path: string, body?: unknown) => Promise<any>;

export const DEMO_NAME = 'Fernbird Consulting (demo)';

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const pad = (n: number) => String(n).padStart(2, '0');
const date = (y: number, m: number, d: number) => {
  const t = new Date(Date.UTC(y, m - 1, d));
  return `${t.getUTCFullYear()}-${pad(t.getUTCMonth() + 1)}-${pad(t.getUTCDate())}`;
};
const lastDay = (y: number, m: number) => new Date(Date.UTC(y, m, 0)).getUTCDate();
const shift = (iso: string, days: number) => {
  const [y, m, d] = iso.split('-').map(Number) as [number, number, number];
  return date(y, m, d + days);
};
/** Each month from April of `start` while `until(y, m)` holds. */
function* months(start: number, until: (y: number, m: number) => boolean) {
  for (let y = start, m = 4; until(y, m); m === 12 ? ((y += 1), (m = 1)) : (m += 1)) yield [y, m] as const;
}

/**
 * Creates the demo business and fills it, reporting progress from 0 to 1. Returns the business's id.
 * The business is created first, so a failure part-way leaves a demo that can be reset.
 */
export async function createDemoBusiness(
  send: DemoCall,
  onProgress: (fraction: number) => void = () => {},
  name: string = DEMO_NAME,
): Promise<string> {
  // A small deterministic generator, so every demo has the same shape.
  let seed = 20240401;
  const random = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
  const between = (lo: number, hi: number) => lo + Math.floor(random() * (hi - lo + 1));

  let calls = 0;
  let expected = 400;
  const call: DemoCall = async (method, path, body) => {
    const result: unknown = await send(method, path, body);
    calls += 1;
    onProgress(Math.min(0.99, calls / expected));
    return result;
  };

  const business = await call('POST', '/businesses', {
    name,
    trading_name: 'Fernbird Consulting',
    business_type: 'contractor',
    email: 'hello@fernbird.example',
    gst_registered: true,
    gst_number: '49091850',
    gst_registered_from: '2020-04-01',
    gst_basis: 'payments',
    gst_period: 'two_monthly',
    gst_period_end_month: 3,
    reporting_basis: 'accrual',
    complete_onboarding: true,
    demo: true,
  });
  const b = `/businesses/${business.id as string}`;
  const today: string = business.today;
  const [ty, tm] = today.split('-').map(Number) as [number, number];
  // The records start with last financial year (1 April) and run to today.
  const start = Number((business.current_financial_year.start as string).slice(0, 4)) - 1;
  const opening = date(start, 3, 31);
  const finished = [date(start + 1, 3, 31)];
  const monthCount = (ty - start) * 12 + tm - 4;
  expected = 70 + monthCount * 11;

  // Opening balances from a (made-up) accountant's balance sheet.
  await call('PUT', `${b}/opening-balances`, {
    as_at: opening,
    bank: 1_284_000,
    gst_payable: 296_500,
    drawings: -9_640_000,
    funds_introduced: 750_000,
    withholding_tax: -4_310_000,
    income_tax: 0,
  });
  await call('POST', `${b}/assets`, {
    name: 'Laptop (older)',
    asset_group: 'Computer equipment',
    cost: 312_000,
    acquired_on: date(start - 2, 9, 12),
    method: 'dv',
    rate: '50',
    opening_value: 117_000,
  });

  const agency = await call('POST', `${b}/customers`, {
    name: 'Harbour Talent Partners (demo)',
    email: 'contractors@harbourtalent.example',
    notes: 'Recruitment agency; the contract is with their client. They withhold 20% schedular tax.',
  });
  const direct = await call('POST', `${b}/customers`, { name: 'Tūī Analytics (demo)', email: 'accounts@tui-analytics.example' });
  const studio = await call('POST', `${b}/customers`, { name: 'Ridgeline Studio (demo)', email: 'studio@ridgeline.example' });

  const line = (description: string, quantity: string, unit_price: number) => ({
    description,
    quantity,
    unit_price,
    tax_treatment: 'standard',
  });
  type Line = ReturnType<typeof line>;
  async function invoice(customer: { id: string }, issue: string, due: string, lines: Line[], reference = '') {
    const draft = await call('POST', `${b}/invoices`, { customer_id: customer.id, issue_date: issue, due_date: due, lines, reference });
    return call('POST', `${b}/invoices/${draft.id as string}/issue`, {});
  }

  // Monthly agency invoices for hours worked, to the last full month; paid on the 20th less 20%
  // withholding tax on the GST-exclusive amount (the payment settles the whole invoice).
  const RATE = 11_500; // $115 an hour
  for (const [y, m] of months(start, (y, m) => y < ty || (y === ty && m < tm))) {
    const hours = m === 12 || m === 1 ? between(112, 136) : between(144, 176);
    const [iy, im] = m === 12 ? [y + 1, 1] : [y, m + 1];
    const due = date(iy, im, 20);
    const inv = await invoice(
      agency,
      date(iy, im, 1),
      due,
      [line(`Data engineering services, ${MONTHS[m - 1]} ${y} (${hours} hours)`, String(hours), RATE)],
      `HTP-${y}${pad(m)}`,
    );
    if (due <= today) {
      await call('POST', `${b}/invoices/${inv.id as string}/payments`, {
        payment_date: due,
        amount: inv.total,
        withholding_tax: Math.round((inv.subtotal as number) * 0.2),
        method: 'bank_transfer',
      });
    }
  }

  // Direct jobs: two paid, one part paid and overdue, one draft; and a quote.
  const workshop = await invoice(direct, date(start, 10, 10), date(start, 11, 10), [line('Dashboard review workshop', '1', 180_000)]);
  await call('POST', `${b}/invoices/${workshop.id as string}/payments`, {
    payment_date: date(start, 11, 6),
    amount: workshop.total,
    method: 'bank_transfer',
  });
  const pipeline = await invoice(direct, shift(today, -75), shift(today, -45), [line('Data pipeline health check', '12', 13_500)]);
  await call('POST', `${b}/invoices/${pipeline.id as string}/payments`, {
    payment_date: shift(today, -46),
    amount: pipeline.total,
    method: 'bank_transfer',
  });
  const report = await invoice(direct, shift(today, -25), shift(today, -11), [
    line('Reporting model redesign', '18', 13_500),
    line('Handover documentation', '4', 13_500),
  ]);
  await call('POST', `${b}/invoices/${report.id as string}/payments`, {
    payment_date: shift(today, -5),
    amount: 150_000,
    method: 'bank_transfer',
  });
  await call('POST', `${b}/invoices`, {
    customer_id: direct.id,
    issue_date: today,
    lines: [line('Follow-up support (hours)', '6', 13_500)],
  });
  await call('POST', `${b}/quotes`, {
    customer_id: studio.id,
    issue_date: today,
    lines: [line('Customer analytics set-up', '1', 420_000), line('Training session', '2', 30_000)],
  });

  // Expenses.
  const categories: { id: string; name: string }[] = await call('GET', `${b}/expense-categories`);
  const category = (n: string) => (categories.find((c) => c.name === n) ?? categories.find((c) => c.name === 'Other') ?? categories[0])?.id;
  const expense = (body: Record<string, unknown>) =>
    call('POST', `${b}/expenses`, { classification: 'business', gst_treatment: 'standard', payment_method: 'business_account', ...body });
  for (const [y, m] of months(start, (y, m) => date(y, m, 28) <= today)) {
    await expense({
      expense_date: date(y, m, 28),
      supplier: 'Bank',
      category_id: category('Bank fees'),
      gross_amount: 800,
      gst_treatment: 'exempt',
    });
    await expense({
      expense_date: date(y, m, 14),
      supplier: 'Mobile provider',
      category_id: category('Telephone'),
      gross_amount: 6_500,
      classification: 'partial',
      business_percentage: '70',
    });
    await expense({
      expense_date: date(y, m, 9),
      supplier: 'Home broadband',
      category_id: category('Internet'),
      gross_amount: 9_500,
      payment_method: 'personal_funds',
      classification: 'partial',
      business_percentage: '35',
    });
    await expense({
      expense_date: date(y, m, 3),
      supplier: 'Cloud tools subscription',
      category_id: category('Software'),
      gross_amount: between(4_200, 6_900),
      payment_method: 'credit_card',
    });
    if (m === 7) {
      await expense({
        expense_date: date(y, 7, 18),
        supplier: 'Accountant',
        category_id: category('Professional services'),
        gross_amount: 138_000,
      });
      await expense({
        expense_date: date(y, 7, 2),
        supplier: 'Professional indemnity insurer',
        category_id: category('Insurance'),
        gross_amount: 115_000,
      });
    }
    if (m % 3 === 0) {
      await expense({
        expense_date: date(y, m, 22),
        supplier: 'Café',
        description: 'Lunch with a client',
        category_id: category('Meals'),
        gross_amount: between(9_000, 16_000),
        deductible_percentage: '50',
      });
    }
  }
  await expense({
    expense_date: date(start, 8, 16),
    supplier: 'Private seller',
    description: 'Secondhand standing desk (3/23 GST claim)',
    category_id: category('Office supplies'),
    gross_amount: 46_000,
    payment_method: 'personal_funds',
  });
  await expense({
    expense_date: date(start + 1, 2, 3),
    supplier: 'Conference organiser',
    description: 'Data conference ticket',
    category_id: category('Travel'),
    gross_amount: 89_000,
  });

  // Interest on the business account.
  for (const [y, m] of months(start, (y, m) => date(y, m, lastDay(y, m)) <= today)) {
    await call('POST', `${b}/income`, {
      income_date: date(y, m, lastDay(y, m)),
      payer: 'Bank',
      description: 'Interest',
      income_type: 'other',
      gross_amount: between(300, 1_400),
      gst_treatment: 'exempt',
      payment_method: 'bank_transfer',
    });
  }

  // Assets: a new laptop, and a monitor deducted in full as a low-value asset.
  await call('POST', `${b}/assets`, {
    name: 'Laptop',
    asset_group: 'Computer equipment',
    cost: 278_174,
    purchase_gst: 41_726,
    acquired_on: date(start, 6, 14),
    method: 'dv',
    rate: '50',
  });
  await call('POST', `${b}/assets`, {
    name: 'Monitor',
    asset_group: 'Computer equipment',
    cost: 52_174,
    purchase_gst: 7_826,
    acquired_on: date(start, 10, 4),
    method: 'full',
    rate: '100',
  });

  // Home office by IRD's square metre rate, for the finished year.
  for (const year_end of finished) {
    await call('PUT', `${b}/home-office`, {
      year_end,
      business_area: '11.5',
      total_area: '128',
      premises_costs: 2_600_000,
      notes: 'Rent for the year; spare room used as an office.',
    });
  }

  // Mileage: one car with IRD-style petrol rates and tiers; client visits, and monthly private totals
  // so the tiers follow the car's whole travel.
  const vehicle = await call('POST', `${b}/mileage/vehicles`, {
    name: 'Hatchback',
    registration: 'DEMO1',
    make: 'Mazda',
    model: '3',
    year: 2018,
    vehicle_type: 'car',
    fuel_type: 'petrol',
    new_rate: {
      name: 'Petrol rate',
      effective_from: date(start, 4, 1),
      rate_per_km: '1.17',
      tier1_limit_km: '14000',
      tier2_rate_per_km: '0.37',
      notes: 'Kilometre rates for a petrol car (demo figures).',
    },
  });
  await call('POST', `${b}/mileage/rates/${vehicle.default_rate_id as string}/versions`, {
    effective_from: date(start + 1, 4, 1),
    rate_per_km: '1.20',
    tier1_limit_km: '14000',
    tier2_rate_per_km: '0.37',
  });
  for (const [y, m] of months(start, (y, m) => date(y, m, 1) <= today)) {
    for (const day of [6, 20]) {
      if (date(y, m, day) > today) continue;
      await call('POST', `${b}/mileage/trips`, {
        vehicle_id: vehicle.id,
        trip_date: date(y, m, day),
        purpose: 'On-site day at the client',
        start_location: 'Home',
        destination: 'Client office',
        distance_km: String(between(36, 44) * 2),
      });
    }
    if (date(y, m, lastDay(y, m)) > today) continue;
    await call('POST', `${b}/mileage/trips`, {
      vehicle_id: vehicle.id,
      trip_date: date(y, m, lastDay(y, m)),
      purpose: 'Private travel this month',
      distance_km: String(between(1_750, 2_050)),
      business_percentage: '0',
    });
  }

  // Drawings each month, and depreciation for the finished year.
  for (const [y, m] of months(start, (y, m) => date(y, m, 25) <= today)) {
    await call('POST', `${b}/cash-movements`, {
      kind: 'drawing',
      movement_date: date(y, m, 25),
      amount: between(10, 13) * 50_000,
      description: 'Transfer to personal account',
    });
  }
  for (const year_end of finished) await call('POST', `${b}/assets/depreciation`, { year_end });

  // Pay IRD each GST return that's due, on its due date (and what was owed at the opening balance).
  const gst = await call('GET', `${b}/gst/returns`);
  if (gst.brought_forward?.outstanding > 0) {
    await call('POST', `${b}/cash-movements`, {
      kind: 'gst_payment',
      movement_date: date(start, 5, 7),
      amount: gst.brought_forward.outstanding,
      description: `GST owed at ${opening}`,
    });
  }
  const returns = (gst.returns as { due_date: string; end: string; net: number }[])
    .filter((r) => r.due_date < today)
    .sort((x, y) => x.end.localeCompare(y.end));
  for (const r of returns) {
    if (r.net === 0) continue;
    await call('POST', `${b}/cash-movements`, {
      kind: r.net > 0 ? 'gst_payment' : 'gst_refund',
      movement_date: r.due_date,
      amount: Math.abs(r.net),
      description: `GST return to ${r.end}`,
    });
  }

  // Allocation rules (planning only), so the next payments get a proposed split.
  const destinations: { id: string; name: string }[] = await call('GET', `${b}/allocations/destinations`);
  const destination = (n: string) => destinations.find((d) => d.name === n)?.id;
  const gstPot = destination('GST');
  if (gstPot) await call('POST', `${b}/allocations/rules`, { destination_id: gstPot, method: 'percentage', percentage: '13.04' });
  const emergency = destination('Emergency fund');
  if (emergency) {
    await call('PUT', `${b}/allocations/destinations/${emergency}`, {
      name: 'Emergency fund',
      kind: 'emergency_fund',
      target_amount: 1_500_000,
      minimum_balance: 500_000,
    });
    await call('POST', `${b}/allocations/rules`, {
      destination_id: emergency,
      method: 'fixed',
      fixed_amount: 50_000,
      stop_at_target: true,
    });
  }
  // One more payment on the part-paid job, after the rules, so there's an allocation to review.
  await call('POST', `${b}/invoices/${report.id as string}/payments`, { payment_date: today, amount: 50_000, method: 'bank_transfer' });

  onProgress(1);
  return business.id as string;
}
