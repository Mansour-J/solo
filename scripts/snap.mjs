#!/usr/bin/env node
// Dev utility: screenshots app routes (light, dark or phone) and reports console and page errors.
//
//   npm run snap -- [--dark] [--mobile] [--empty] [--fresh] [--full] [name:]/route ...   (--full: the whole page)
//   npm run snap -- --dark /dashboard /invoices/{invoice} 'expense-form:/expenses?new=1'
//
// Needs the Vite dev server (default http://localhost:5173) proxying an API, e.g. `npm run local:serve`
// with `VITE_DEV_API_PROXY=<its URL> npm run dev`. It creates data, so it only runs against localhost:
//   * a business called "Snapshot test business" (there is no sign-in: it belongs to the server's owner),
//     saved in /tmp/solo-snaps/business.json and reused by later runs (--fresh makes a new one);
//   * test records created through the API (customers, invoices, a payment, a quote, expenses, income,
//     a vehicle with a rate and trips, allocation rules), every one labelled "(test)". They exist only to fill the screens:
//     never present them as real.
// --empty uses a new business with no records, for reviewing empty states.
// Routes may use the seeded IDs: {customer}, {invoice} (issued, part paid), {draft}, {quote}, {expense}, {vehicle},
// {allocation} (waiting for review).
// Quote routes containing `?` for the shell. Screenshots go to /tmp/solo-snaps/<name>[_m][_d].png, or to
// SNAP_OUT_DIR.
/* global document, window */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { chromium, devices } from '@playwright/test';

const FLAGS = ['--dark', '--mobile', '--empty', '--fresh', '--full'];
const args = process.argv.slice(2);
const unknown = args.filter((a) => a.startsWith('--') && !FLAGS.includes(a));
if (unknown.length) {
  console.error(`Unknown option ${unknown.join(', ')}. Options: ${FLAGS.join(' ')}`);
  process.exit(2);
}
const dark = args.includes('--dark');
const fullPage = args.includes('--full');
const mobile = args.includes('--mobile');
const empty = args.includes('--empty');
const fresh = empty || args.includes('--fresh');
const specs = args.filter((a) => !a.startsWith('--'));

const base = (process.env.SNAP_BASE_URL ?? 'http://localhost:5173').replace(/\/$/, '');
if (!['localhost', '127.0.0.1'].includes(new URL(base).hostname)) {
  console.error(`Refusing to create test data on ${base}: snap.mjs only runs against a local stack.`);
  process.exit(2);
}
const stateDir = '/tmp/solo-snaps';
const businessFile = `${stateDir}/business.json`;
// Screenshots go here; SNAP_OUT_DIR overrides it (scripts/readme-screenshots.mjs writes docs/screenshots).
const outDir = process.env.SNAP_OUT_DIR ?? stateDir;
mkdirSync(stateDir, { recursive: true });
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const context = await browser.newContext({
  ...(mobile ? devices['Pixel 7'] : { viewport: { width: 1440, height: 900 } }),
  deviceScaleFactor: 1,
  colorScheme: dark ? 'dark' : 'light',
});

/** Calls the API. */
async function call(method, path, data) {
  const response = await context.request.fetch(`${base}/api/v1${path}`, { method, data, headers: { 'x-client-kind': 'web' } });
  const body = response.status() === 204 ? null : await response.json().catch(() => null);
  return { ok: response.ok(), status: response.status(), body };
}

async function must(method, path, data) {
  const { ok, status, body } = await call(method, path, data);
  if (!ok) throw new Error(`${method} ${path} failed (${status}): ${JSON.stringify(body)}`);
  return body;
}

/** YYYY-MM-DD, `days` from the business's today. */
function shift(today, days) {
  const date = new Date(`${today}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

async function issuedInvoice(b, customerId, issueDate, dueDate, lines) {
  const draft = await must('POST', `/businesses/${b}/invoices`, {
    customer_id: customerId,
    issue_date: issueDate,
    due_date: dueDate,
    lines,
  });
  return must('POST', `/businesses/${b}/invoices/${draft.id}/issue`, {});
}

/** Fills the business with clearly labelled test records; returns the IDs routes can refer to. */
async function seed(b, today) {
  const line = (description, quantity, unit_price) => ({ description, quantity, unit_price, tax_treatment: 'standard' });
  const customer = (name, email) => must('POST', `/businesses/${b}/customers`, { name, email });
  const kereru = await customer('Kererū Café (test)', 'accounts@kereru.example');
  const pukeko = await customer('Pūkeko Builders (test)', 'office@pukeko.example');
  const totara = await customer('Tōtara Design and Architecture Studio Limited (test)', 'hello@totara.example');

  // Allocation rules with test figures of the user's own, so the payments below get allocations.
  const destinations = await must('GET', `/businesses/${b}/allocations/destinations`);
  const destination = (name) => destinations.find((d) => d.name === name).id;
  await must('PUT', `/businesses/${b}/allocations/destinations/${destination('Emergency fund')}`, {
    name: 'Emergency fund',
    kind: 'emergency_fund',
    target_amount: 300_000,
    minimum_balance: 100_000,
  });
  await must('POST', `/businesses/${b}/allocations/rules`, {
    destination_id: destination('Income tax'),
    method: 'percentage',
    percentage: '20',
    notes: '(test)',
  });
  await must('POST', `/businesses/${b}/allocations/rules`, {
    destination_id: destination('Emergency fund'),
    method: 'fixed',
    fixed_amount: 10_000,
    stop_at_target: true,
    notes: '(test)',
  });

  const paid = await issuedInvoice(b, kereru.id, shift(today, -40), shift(today, -26), [line('Brand workshop (test)', '4', 25_000)]);
  await must('POST', `/businesses/${b}/invoices/${paid.id}/payments`, {
    payment_date: shift(today, -30),
    amount: paid.total,
    method: 'bank_transfer',
  });
  const overdue = await issuedInvoice(b, pukeko.id, shift(today, -25), shift(today, -5), [
    line('Site plans (test)', '1', 180_000),
    line('Consultation (test)', '2.5', 12_000),
  ]);
  await must('POST', `/businesses/${b}/invoices/${overdue.id}/payments`, {
    payment_date: shift(today, -3),
    amount: 50_000,
    method: 'bank_transfer',
  });
  const draft = await must('POST', `/businesses/${b}/invoices`, {
    customer_id: totara.id,
    issue_date: shift(today, -1),
    lines: [line('Website copy (test)', '6', 9_500)],
  });
  const quote = await must('POST', `/businesses/${b}/quotes`, {
    customer_id: kereru.id,
    issue_date: shift(today, -2),
    lines: [line('Menu redesign (test)', '1', 120_000)],
  });

  const categories = await must('GET', `/businesses/${b}/expense-categories`);
  const category = (name) => (categories.find((c) => c.name === name) ?? categories[0]).id;
  const expense = await must('POST', `/businesses/${b}/expenses`, {
    expense_date: shift(today, -12),
    supplier: 'Spark (test)',
    category_id: category('Telephone'),
    gross_amount: 11_500,
    gst_treatment: 'standard',
    payment_method: 'business_account',
    classification: 'business',
  });
  await must('POST', `/businesses/${b}/expenses`, {
    expense_date: shift(today, -6),
    supplier: 'Design software subscription (test)',
    category_id: category('Software'),
    gross_amount: 8_999,
    gst_treatment: 'standard',
    payment_method: 'credit_card',
    classification: 'partial',
    business_percentage: '80',
  });
  await must('POST', `/businesses/${b}/income`, {
    income_date: shift(today, -8),
    payer: 'Market stall (test)',
    income_type: 'sales',
    gross_amount: 23_000,
    gst_treatment: 'standard',
    payment_method: 'cash',
  });
  // A vehicle with a test rate of the user's own (no government rate is implied) and three trips.
  const vehicle = await must('POST', `/businesses/${b}/mileage/vehicles`, {
    name: 'Work car (test)',
    registration: 'TEST1',
    make: 'Toyota',
    model: 'Corolla',
    year: 2019,
    vehicle_type: 'car',
    fuel_type: 'hybrid',
    new_rate: { name: 'Kilometre rate (test)', effective_from: shift(today, -120), rate_per_km: '1.00' },
  });
  const trip = (days, purpose, body) =>
    must('POST', `/businesses/${b}/mileage/trips`, { vehicle_id: vehicle.id, trip_date: shift(today, days), purpose, ...body });
  await trip(-60, 'Site visit (test)', { start_location: 'Home', destination: 'Pūkeko Builders', distance_km: '84.6' });
  await trip(-30, 'Client workshop (test)', { destination: 'Kererū Café', start_odometer: '10000', end_odometer: '10042.5' });
  await trip(-5, 'Supplies and personal errand (test)', { distance_km: '36', business_percentage: '50' });
  // Opening balances at the end of last financial year, as an accountant's balance sheet would give them.
  const { current_financial_year: fy } = await must('GET', `/businesses/${b}`);
  await must('PUT', `/businesses/${b}/opening-balances`, {
    as_at: shift(fy.start, -1),
    bank: 1_850_000,
    gst_payable: 120_000,
    drawings: -4_200_000,
    funds_introduced: 500_000,
  });
  // Year end: money taken out and paid to IRD, and equipment to depreciate.
  for (const [days, kind, amount, description] of [
    [-45, 'drawing', 150_000, 'Transfer to personal account (test)'],
    [-20, 'gst_payment', 38_000, 'GST return (test)'],
    [-15, 'drawing', 150_000, 'Transfer to personal account (test)'],
  ]) {
    await must('POST', `/businesses/${b}/cash-movements`, { movement_date: shift(today, days), kind, amount, description });
  }
  for (const [name, cost, rate, days] of [
    ['Laptop (test)', 240_000, '50', -90],
    ['Camera (test)', 180_000, '40', -50],
  ]) {
    await must('POST', `/businesses/${b}/assets`, {
      name,
      asset_group: 'Computer equipment',
      cost,
      purchase_gst: cost * 0.15,
      acquired_on: shift(today, days),
      method: 'dv',
      rate,
    });
  }
  // Confirm the oldest allocation as planned (balances and history); the rest stay to review.
  const pending = await must('GET', `/businesses/${b}/allocations?status=pending&per_page=100`);
  const oldest = await must('GET', `/businesses/${b}/allocations/${pending.items.at(-1).id}`);
  await must('POST', `/businesses/${b}/allocations/${oldest.id}/confirm`, {
    lines: oldest.plan.lines.map((l) => ({ destination_id: l.destination_id, amount: l.amount, rule_id: l.rule_id })),
  });
  return {
    customer: kereru.id,
    invoice: overdue.id,
    draft: draft.id,
    quote: quote.id,
    expense: expense.id,
    vehicle: vehicle.id,
    allocation: pending.items[0].id,
  };
}

/** The saved test business, or a new (seeded) one; the browser opens on it. */
async function business() {
  const saved = !fresh && existsSync(businessFile) ? JSON.parse(readFileSync(businessFile, 'utf8')) : null;
  let chosen = null;
  if (saved?.base === base) {
    const found = await call('GET', `/businesses/${saved.id}`);
    if (found.ok) chosen = saved;
    else console.log(`Saved test business is gone (${found.status}); creating a new one.`);
  }
  if (!chosen) {
    const created = await must('POST', '/businesses', {
      name: 'Snapshot test business',
      gst_registered: true,
      gst_number: '49091850',
      gst_registered_from: '2020-01-01',
      complete_onboarding: true,
    });
    chosen = { base, id: created.id, ids: empty ? {} : await seed(created.id, created.today) };
    if (!empty) writeFileSync(businessFile, `${JSON.stringify(chosen, null, 2)}\n`);
    console.log(`${empty ? 'Empty' : 'Seeded'} test business ${chosen.id}`);
  }
  await context.addInitScript((id) => window.localStorage.setItem('selected-business', id), chosen.id);
  return chosen;
}

const problems = [];
try {
  const health = await context.request.get(`${base}/health/ready`).catch(() => null);
  if (!health?.ok()) {
    throw new Error(
      `${base}/health/ready doesn't answer 200: start the API (npm run local:serve) and the web app (VITE_DEV_API_PROXY=<its URL> npm run dev) first.`,
    );
  }
  const { ids } = await business();
  const page = await context.newPage();
  page.on('console', (msg) => {
    if (msg.type() === 'error' || msg.type() === 'warning') problems.push(`[${msg.type()}] ${msg.text().slice(0, 400)}`);
  });
  page.on('pageerror', (err) => problems.push(`[pageerror] ${err.message}`));

  for (const spec of specs.length ? specs : ['/dashboard']) {
    const [, label, template] = /^(?:([\w-]+):)?(\/.*)$/.exec(spec) ?? [];
    if (!template) throw new Error(`Routes start with "/" (optionally "name:/route"), got ${spec}`);
    const route = template.replace(/\{(\w+)\}/g, (_, key) => {
      if (!ids[key]) throw new Error(`No seeded ${key} for ${spec}${empty ? ' (--empty has no records)' : ''}`);
      return ids[key];
    });
    const name = label ?? (template.replace(/[^a-z0-9]+/gi, '_').replace(/^_+|_+$/g, '') || 'root');
    await page.goto(`${base}/#${route}`);
    // Wait until the app has rendered and its loaders and skeletons are gone (a hot reload from a
    // concurrent edit can restart the page).
    const ready = () =>
      (document.getElementById('root')?.childElementCount ?? 0) > 0 &&
      !document.querySelector('[aria-busy="true"], [aria-label="Loading"]');
    await page.waitForFunction(ready, null, { timeout: 20_000 }).catch(() => {
      problems.push(`[snap] ${route} still showed a loader after 20 s`);
    });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(800);
    // Nothing should make the page scroll sideways (tables scroll inside their own containers).
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    if (overflow > 1) problems.push(`[snap] ${route} is ${overflow}px wider than the screen (it scrolls sideways)`);
    const file = `${outDir}/${name}${mobile ? '_m' : ''}${dark ? '_d' : ''}.png`;
    await page.screenshot({ path: file, fullPage });
    console.log('saved', file);
  }
} catch (error) {
  problems.push(`[snap] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
} finally {
  await browser.close();
}
console.log(problems.length ? `PROBLEMS:\n${[...new Set(problems)].join('\n')}` : 'no console problems');
