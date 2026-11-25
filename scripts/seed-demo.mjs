#!/usr/bin/env node
// Adds a made-up demo business, "Fernbird Consulting (demo)", with about two and a half years of
// records shaped like a contractor's: monthly invoices to a recruitment agency with tax withheld,
// GST every two months on the payments basis, expenses, assets, home office, mileage with IRD's two
// tiers, drawings and allocation rules. Every name and figure is invented, for demonstrating Solo.
//
//   SEED_BASE_URL=http://127.0.0.1:<port> node scripts/seed-demo.mjs   straight to an API (e.g. solo-local)
//   node scripts/seed-demo.mjs                                         through Vite (http://localhost:5173)
//
// There is no sign-in: the business is added for the server's owner, next to their own businesses.
// A server that requires a key (the desktop app's own) needs SEED_API_KEY.

const base = (process.env.SEED_BASE_URL ?? 'http://localhost:5173').replace(/\/$/, '');
const NAME = process.env.SEED_DEMO_NAME ?? 'Fernbird Consulting (demo)';

async function call(method, path, body) {
  const response = await fetch(`${base}/api/v1${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      'x-client-kind': 'web',
      ...(process.env.SEED_API_KEY ? { 'x-api-key': process.env.SEED_API_KEY } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = response.status === 204 ? null : await response.json().catch(() => null);
  if (!response.ok) throw new Error(`${method} ${path} failed (${response.status}): ${JSON.stringify(json)}`);
  return json;
}

const pad = (n) => String(n).padStart(2, '0');
const date = (y, m, d) => {
  const t = new Date(Date.UTC(y, m - 1, d));
  return `${t.getUTCFullYear()}-${pad(t.getUTCMonth() + 1)}-${pad(t.getUTCDate())}`;
};
const lastDay = (y, m) => new Date(Date.UTC(y, m, 0)).getUTCDate();
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
// A small deterministic generator, so every run makes the same demo.
let seed = 20240401;
const random = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
const between = (lo, hi) => lo + Math.floor(random() * (hi - lo + 1));

const existing = await call('GET', '/businesses');
if ((existing.items ?? existing).some((b) => b.name === NAME)) {
  console.log(`${NAME} already exists; nothing to do.`);
  process.exit(0);
}

const business = await call('POST', '/businesses', {
  name: NAME,
  business_type: 'contractor',
  email: 'hello@fernbird.example',
  gst_registered: true,
  gst_number: '49091850',
  gst_registered_from: '2021-04-01',
  gst_basis: 'payments',
  gst_period: 'two_monthly',
  gst_period_end_month: 3,
  reporting_basis: 'accrual',
  complete_onboarding: true,
});
const b = `/businesses/${business.id}`;
const today = business.today;
console.log(`Added ${NAME} (${business.id}); today is ${today}.`);

// Opening balances from a (made-up) accountant's balance sheet at 31 March 2024.
await call('PUT', `${b}/opening-balances`, {
  as_at: '2024-03-31',
  bank: 1_284_000,
  gst_payable: 296_500,
  drawings: -9_640_000,
  funds_introduced: 750_000,
  withholding_tax: -4_310_000,
  income_tax: 0,
});
await call('POST', `${b}/assets`, {
  name: 'Laptop (2022)',
  asset_group: 'Computer equipment',
  cost: 312_000,
  acquired_on: '2022-09-12',
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

const line = (description, quantity, unit_price) => ({ description, quantity, unit_price, tax_treatment: 'standard' });
async function invoice(customer, issue, due, lines, reference = '') {
  const draft = await call('POST', `${b}/invoices`, { customer_id: customer.id, issue_date: issue, due_date: due, lines, reference });
  return call('POST', `${b}/invoices/${draft.id}/issue`, {});
}

// Monthly agency invoices for hours worked, April 2024 to the last full month; paid on the 20th less
// 20% withholding tax on the GST-exclusive amount (the payment settles the whole invoice, tax withheld included).
const RATE = 11_500; // $115 an hour
const [ty, tm] = today.split('-').map(Number);
let invoices = 0;
for (let y = 2024, m = 4; y < ty || (y === ty && m < tm); m === 12 ? ((y += 1), (m = 1)) : (m += 1)) {
  const hours = m === 12 || m === 1 ? between(112, 136) : between(144, 176);
  const [iy, im] = m === 12 ? [y + 1, 1] : [y, m + 1];
  const issue = date(iy, im, 1);
  const due = date(iy, im, 20);
  const inv = await invoice(
    agency,
    issue,
    due,
    [line(`Data engineering services, ${MONTHS[m - 1]} ${y} (${hours} hours)`, String(hours), RATE)],
    `HTP-${y}${pad(m)}`,
  );
  if (due <= today) {
    const withheld = Math.round(inv.subtotal * 0.2);
    await call('POST', `${b}/invoices/${inv.id}/payments`, {
      payment_date: due,
      amount: inv.total,
      withholding_tax: withheld,
      method: 'bank_transfer',
    });
  }
  invoices += 1;
}

// A few direct jobs: two paid, one part paid and overdue, one draft; and a quote.
const workshop = await invoice(direct, '2025-02-10', '2025-03-10', [line('Dashboard review workshop', '1', 180_000)]);
await call('POST', `${b}/invoices/${workshop.id}/payments`, {
  payment_date: '2025-03-06',
  amount: workshop.total,
  method: 'bank_transfer',
});
const pipeline = await invoice(direct, '2025-09-15', '2025-10-15', [line('Data pipeline health check', '12', 13_500)]);
await call('POST', `${b}/invoices/${pipeline.id}/payments`, {
  payment_date: '2025-10-14',
  amount: pipeline.total,
  method: 'bank_transfer',
});
const report = await invoice(direct, date(ty, tm - 1, 5), date(ty, tm - 1, 19), [
  line('Reporting model redesign', '18', 13_500),
  line('Handover documentation', '4', 13_500),
]);
await call('POST', `${b}/invoices/${report.id}/payments`, { payment_date: date(ty, tm - 1, 28), amount: 150_000, method: 'bank_transfer' });
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
const categories = await call('GET', `${b}/expense-categories`);
const category = (name) => (categories.find((c) => c.name === name) ?? categories.find((c) => c.name === 'Other')).id;
const expense = (body) =>
  call('POST', `${b}/expenses`, { classification: 'business', gst_treatment: 'standard', payment_method: 'business_account', ...body });
let expenses = 0;
for (let y = 2024, m = 4; date(y, m, 28) <= today; m === 12 ? ((y += 1), (m = 1)) : (m += 1)) {
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
  expenses += 4;
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
    expenses += 2;
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
    expenses += 1;
  }
}
await expense({
  expense_date: '2025-08-16',
  supplier: 'Private seller',
  description: 'Secondhand standing desk (3/23 GST claim)',
  category_id: category('Office supplies'),
  gross_amount: 46_000,
  payment_method: 'personal_funds',
});
await expense({
  expense_date: '2026-02-03',
  supplier: 'Conference organiser',
  description: 'Data conference ticket',
  category_id: category('Travel'),
  gross_amount: 89_000,
});
expenses += 2;

// Interest on the business account.
for (let y = 2024, m = 4; date(y, m, lastDay(y, m)) <= today; m === 12 ? ((y += 1), (m = 1)) : (m += 1)) {
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
  name: 'Laptop (2025)',
  asset_group: 'Computer equipment',
  cost: 278_174,
  purchase_gst: 41_726,
  acquired_on: '2025-06-14',
  method: 'dv',
  rate: '50',
});
await call('POST', `${b}/assets`, {
  name: 'Monitor',
  asset_group: 'Computer equipment',
  cost: 52_174,
  purchase_gst: 7_826,
  acquired_on: '2024-10-04',
  method: 'full',
  rate: '100',
});

// Home office by IRD's square metre rate, for each finished year.
for (const year_end of ['2025-03-31', '2026-03-31']) {
  await call('PUT', `${b}/home-office`, {
    year_end,
    business_area: '11.5',
    total_area: '128',
    premises_costs: year_end === '2025-03-31' ? 2_496_000 : 2_600_000,
    notes: 'Rent for the year; spare room used as an office.',
  });
}

// Mileage: one car with IRD's petrol rates and tiers; client visits, and monthly private totals so the
// tiers follow the car's whole travel.
const vehicle = await call('POST', `${b}/mileage/vehicles`, {
  name: 'Hatchback',
  registration: 'DEMO1',
  make: 'Mazda',
  model: '3',
  year: 2018,
  vehicle_type: 'car',
  fuel_type: 'petrol',
  new_rate: {
    name: 'IRD petrol',
    effective_from: '2024-04-01',
    rate_per_km: '1.17',
    tier1_limit_km: '14000',
    tier2_rate_per_km: '0.37',
    notes: 'IRD kilometre rates for petrol cars.',
  },
});
await call('POST', `${b}/mileage/rates/${vehicle.default_rate_id}/versions`, {
  effective_from: '2025-04-01',
  rate_per_km: '1.20',
  tier1_limit_km: '14000',
  tier2_rate_per_km: '0.37',
});
let trips = 0;
for (let y = 2024, m = 4; date(y, m, 1) <= today; m === 12 ? ((y += 1), (m = 1)) : (m += 1)) {
  for (const day of [6, 20]) {
    if (date(y, m, day) > today) continue;
    trips += 1;
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
  trips += 1;
}

// Drawings each month, and post depreciation for each finished year.
for (let y = 2024, m = 4; date(y, m, 25) <= today; m === 12 ? ((y += 1), (m = 1)) : (m += 1)) {
  await call('POST', `${b}/cash-movements`, {
    kind: 'drawing',
    movement_date: date(y, m, 25),
    amount: between(10, 13) * 50_000,
    description: 'Transfer to personal account',
  });
}
for (const year_end of ['2025-03-31', '2026-03-31']) await call('POST', `${b}/assets/depreciation`, { year_end });

// Pay IRD each GST return that's due, on its due date (and what was owed at the opening balance).
const gst = await call('GET', `${b}/gst/returns`);
if (gst.brought_forward?.outstanding > 0) {
  await call('POST', `${b}/cash-movements`, {
    kind: 'gst_payment',
    movement_date: '2024-05-07',
    amount: gst.brought_forward.outstanding,
    description: 'GST owed at 31 March 2024',
  });
}
const due = gst.returns.filter((r) => r.due_date < today).sort((a, c) => a.end.localeCompare(c.end));
let paidReturns = 0;
for (const r of due) {
  if (r.net === 0) continue;
  await call('POST', `${b}/cash-movements`, {
    kind: r.net > 0 ? 'gst_payment' : 'gst_refund',
    movement_date: r.due_date,
    amount: Math.abs(r.net),
    description: `GST return to ${r.end}`,
  });
  paidReturns += 1;
}

// Allocation rules (planning only), so the next payments get a proposed split.
const destinations = await call('GET', `${b}/allocations/destinations`);
const destination = (name) => destinations.find((d) => d.name === name)?.id;
if (destination('GST'))
  await call('POST', `${b}/allocations/rules`, { destination_id: destination('GST'), method: 'percentage', percentage: '13.04' });
if (destination('Emergency fund')) {
  await call('PUT', `${b}/allocations/destinations/${destination('Emergency fund')}`, {
    name: 'Emergency fund',
    kind: 'emergency_fund',
    target_amount: 1_500_000,
    minimum_balance: 500_000,
  });
  await call('POST', `${b}/allocations/rules`, {
    destination_id: destination('Emergency fund'),
    method: 'fixed',
    fixed_amount: 50_000,
    stop_at_target: true,
  });
}
// One more payment on the part-paid job, after the rules, so there's an allocation to review.
await call('POST', `${b}/invoices/${report.id}/payments`, { payment_date: today, amount: 50_000, method: 'bank_transfer' });

console.log(`${invoices + 4} invoices, ${expenses} expenses, ${trips} trips and ${paidReturns} GST returns paid.`);
console.log('Done. Choose the business from the switcher to see it.');
