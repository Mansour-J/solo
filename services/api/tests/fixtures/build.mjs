#!/usr/bin/env node
// Builds the annual-accounts fixtures (fy2025.json, fy2026.json): two years of a made-up sole trader
// (every name and amount is invented), with the figures an accountant's annual report would print.
//
//   node services/api/tests/fixtures/build.mjs
//
// The figures come from simulating Solo's posting rules here, independently of the API (rounding half
// away from zero, GST at 3/23 of inclusive amounts, business shares of partly private costs,
// diminishing-value depreciation). The integration tests (tests/annual_reports.rs) then check that Solo,
// through its API, arrives at the same figures, including each year's comparatives.
import { writeFileSync } from 'node:fs';

const here = new URL('.', import.meta.url);

// ─── Solo's arithmetic ──────────────────────────────────────────────────────

/** Rounds half away from zero, as `domain::money::round_to_minor`. */
const round = (x) => Math.sign(x) * Math.round(Math.abs(x) + 1e-9 * Math.sign(Math.abs(x)));
/** Whole dollars as the reports print them (half away from zero). */
const dollars = (cents) => Math.sign(cents) * Math.floor((Math.abs(cents) + 50) / 100);
const gstIn = (gross) => round((gross * 3) / 23);
const pct = (amount, percent) => round((amount * percent) / 100);

const months = (fyEnd) => {
  const y = Number(fyEnd.slice(0, 4));
  return Array.from({ length: 12 }, (_, i) => {
    const m = ((3 + i) % 12) + 1;
    return `${m >= 4 ? y - 1 : y}-${String(m).padStart(2, '0')}`;
  });
};
const monthName = (m) => new Date(`${m}-01T00:00:00Z`).toLocaleString('en-NZ', { month: 'long', year: 'numeric', timeZone: 'UTC' });

// ─── The ledger, simulated ──────────────────────────────────────────────────

class Books {
  constructor(opening) {
    this.bank = opening.bank;
    this.gst = opening.gst_payable;
    this.equity = { drawings: 0, funds: 0, withholding: 0 };
    this.sales = 0;
    this.expenses = {};
    this.adjustments = {};
  }

  invoice({ lines, payment }) {
    const net = lines.reduce((sum, l) => sum + round(Number(l.quantity) * l.unit_price), 0);
    const gst = lines.reduce((sum, l) => sum + round(round(Number(l.quantity) * l.unit_price) * 0.15), 0);
    if (payment.amount !== net + gst) throw new Error(`payment ${payment.amount} ≠ total ${net + gst}`);
    if (payment.withholding_tax >= payment.amount) throw new Error('withholding must be less than the payment');
    this.sales += net;
    this.gst += gst;
    this.bank += payment.amount - payment.withholding_tax;
    this.equity.withholding -= payment.withholding_tax;
  }

  expense(e, nonDeductible) {
    const gst = e.gst_treatment === 'standard' ? gstIn(e.gross_amount) : 0;
    const share = e.classification === 'partial' ? Number(e.business_percentage) : 100;
    const net = pct(e.gross_amount - gst, share);
    const claim = pct(gst, share);
    const book = nonDeductible.has(e.category) ? this.adjustments : this.expenses;
    book[e.category] = (book[e.category] ?? 0) + net;
    this.gst -= claim;
    if (e.payment_method === 'personal_funds') {
      this.equity.funds += net + claim;
    } else {
      this.bank -= e.gross_amount;
      this.equity.drawings -= e.gross_amount - net - claim;
    }
  }

  movement({ kind, amount }) {
    const sign = { drawing: -1, funds_introduced: 1, gst_payment: -1 }[kind];
    this.bank += sign * amount;
    if (kind === 'drawing') this.equity.drawings -= amount;
    if (kind === 'funds_introduced') this.equity.funds += amount;
    if (kind === 'gst_payment') this.gst -= amount;
  }

  depreciation(total) {
    this.expenses.Depreciation = (this.expenses.Depreciation ?? 0) + total;
  }

  get totalExpenses() {
    return Object.values(this.expenses).reduce((a, b) => a + b, 0);
  }

  get profitBeforeAdjustments() {
    return this.sales - this.totalExpenses;
  }

  get profit() {
    return this.profitBeforeAdjustments - Object.values(this.adjustments).reduce((a, b) => a + b, 0);
  }
}

/** Diminishing value for a year, as `domain::depreciation::year`. */
const depreciate = (assets, opening) =>
  assets.map((a) => {
    const dep = round((opening[a.name] * Number(a.rate)) / 100);
    return { name: a.name, cost: a.cost, opening: opening[a.name], depreciation: dep, closing: opening[a.name] - dep };
  });

const check = (label, cents, expected) => {
  if (dollars(cents) !== expected) throw new Error(`${label}: ${cents / 100} rounds to ${dollars(cents)}, the report says ${expected}`);
};

// ─── Shared records ─────────────────────────────────────────────────────────
// A made-up contractor. Every name and amount is invented; nothing comes from a real business.

const business = { name: 'Fernleaf Consulting', gst_registered: true, gst_number: '49091850', gst_registered_from: '2019-04-01', reporting_basis: 'accrual' };
const opening2024 = {
  as_at: '2024-03-31',
  bank: 4_825_000,
  gst_payable: 213_450,
  drawings: -9_640_000,
  funds_introduced: 1_250_000,
  personal_accounts: -320_000,
  withholding_tax: -5_470_000,
  income_tax: 0,
};
const assets = [
  { name: 'Laptop', asset_group: 'Computer equipment', cost: 329_900, acquired_on: '2022-09-12', method: 'dv', rate: '50', opening_value: 164_950 },
  { name: 'Desktop workstation', asset_group: 'Computer equipment', cost: 245_000, acquired_on: '2021-03-05', method: 'dv', rate: '50', opening_value: 38_281 },
  { name: 'Tablet', asset_group: 'Computer equipment', cost: 149_900, acquired_on: '2023-02-20', method: 'dv', rate: '67', opening_value: 98_934 },
  { name: 'Camera', asset_group: 'Computer equipment', cost: 219_500, acquired_on: '2022-12-02', method: 'dv', rate: '40', opening_value: 131_700 },
];
const nonDeductible = new Set(['Non deductible penalties']);
const business_account = { payment_method: 'business_account', classification: 'business' };

// ─── FY2025: year to 31 March 2025 ──────────────────────────────────────────

const m25 = months('2025-03-31');
const invoices25 = m25.map((m) => ({
  issue_date: `${m}-01`,
  due_date: `${m}-20`,
  lines: [{ description: `Contract services, ${monthName(m)}`, quantity: '1', unit_price: 1_650_000 }],
  // $16,500 + GST, with 20% of the $16,500 withheld by the client (schedular payments).
  payment: { payment_date: `${m}-20`, amount: 1_897_500, withholding_tax: 330_000 },
}));
const office25 = [64_400, 71_250, 58_990, 66_730];
const expenses25 = [
  ...m25.map((m, i) => ({ category: 'Bank fees', supplier: 'Bank', expense_date: `${m}-28`, gross_amount: i < 6 ? 750 : 780, gst_treatment: 'exempt', ...business_account })),
  ...['2024-06-30', '2024-09-30', '2024-12-31', '2025-03-31'].map((d) => ({ category: 'Interest expense', supplier: 'Bank', expense_date: d, gross_amount: 6_120, gst_treatment: 'exempt', ...business_account })),
  ...['2024-06-15', '2024-09-15', '2024-12-15', '2025-03-15'].map((d, i) => ({ category: 'Office expenses', supplier: 'Office costs', expense_date: d, gross_amount: office25[i], gst_treatment: 'standard', ...business_account })),
  // Household internet paid personally; a quarter of it is for the business.
  ...m25.map((m) => ({ category: 'Home Office- Internet', supplier: 'Internet provider', expense_date: `${m}-10`, gross_amount: 8_999, gst_treatment: 'standard', payment_method: 'personal_funds', classification: 'partial', business_percentage: '25' })),
  ...['2024-07-31', '2025-01-31'].map((d) => ({ category: 'Non deductible penalties', supplier: 'Inland Revenue', expense_date: d, gross_amount: 27_500, gst_treatment: 'exempt', ...business_account })),
];
const gstDates25 = ['2024-05-28', '2024-07-28', '2024-09-28', '2024-11-28', '2025-01-28', '2025-03-28'];
const movements25 = [
  ...m25.map((m) => ({ kind: 'drawing', movement_date: `${m}-25`, amount: 1_180_000, description: 'Transfer to personal account' })),
  { kind: 'funds_introduced', movement_date: '2024-08-12', amount: 210_000, description: 'Owner contribution' },
  ...gstDates25.map((d, i) => ({ kind: 'gst_payment', movement_date: d, amount: i === 0 ? opening2024.gst_payable : 520_000, description: 'GST return' })),
];

function year(opening, invoices, expenses, movements, depreciation, gstTarget) {
  const books = new Books(opening);
  invoices.forEach((i) => books.invoice(i));
  expenses.forEach((e) => books.expense(e, nonDeductible));
  books.depreciation(depreciation.reduce((s, a) => s + a.depreciation, 0));
  // The last GST payment settles the return so the year ends owing `gstTarget`.
  const last = movements.filter((m) => m.kind === 'gst_payment').at(-1);
  movements.forEach((m) => books.movement(m));
  const shortfall = books.gst - gstTarget;
  last.amount += shortfall;
  if (last.amount <= 0) throw new Error(`the last GST payment would be ${last.amount}`);
  books.gst -= shortfall;
  books.bank -= shortfall;
  if (books.bank < 0) throw new Error(`the bank account is overdrawn: ${books.bank}`);
  return books;
}

const openingPpe = assets.reduce((s, a) => s + a.opening_value, 0);
const dep25 = depreciate(assets, Object.fromEntries(assets.map((a) => [a.name, a.opening_value])));
const y25 = year(opening2024, invoices25, expenses25, movements25, dep25, 198_400);
const ppe25 = dep25.reduce((s, a) => s + a.closing, 0);
const openingEquity25 = opening2024.bank + openingPpe - opening2024.gst_payable;
const equity25 = {
  drawings: opening2024.drawings + y25.equity.drawings,
  funds: opening2024.funds_introduced + y25.equity.funds,
  withholding: opening2024.withholding_tax + y25.equity.withholding,
};
const retained24 = openingEquity25 - opening2024.drawings - opening2024.funds_introduced - opening2024.personal_accounts - opening2024.withholding_tax;
const closing25 = y25.bank + ppe25 - y25.gst;

// ─── FY2026: year to 31 March 2026 ──────────────────────────────────────────

const m26 = months('2026-03-31');
// Hours at $95 an hour.
const hours = ['152.00', '148.50', '160.25', '158.75', '141.00', '155.50', '162.00', '149.25', '166.75', '112.50', '159.00', '163.50'];
const invoices26 = m26.map((m, i) => {
  const net = round(Number(hours[i]) * 9_500);
  const total = net + round(net * 0.15);
  return {
    issue_date: `${m}-01`,
    due_date: `${m}-20`,
    lines: [{ description: `Contract services, ${monthName(m)} (${Number(hours[i])} hours)`, quantity: hours[i], unit_price: 9_500 }],
    payment: { payment_date: `${m}-20`, amount: total, withholding_tax: round(net * 0.2) },
  };
});

const subscriptions = [
  ['Office suite', 2_990],
  ['Code hosting', 5_800],
  ['AI assistant', 4_900],
  ['Design software', 8_750],
  ['Domain and hosting', 3_990],
  ['Cloud storage', 1_699],
  ['Accounting software', 6_500],
  ['Password manager', 1_250],
  ['Video calls', 2_400],
];
const subs = m26.flatMap((m, i) =>
  subscriptions.map(([supplier, gross]) => ({ category: 'Subscriptions', supplier, expense_date: `${m}-0${(i % 9) + 1}`, gross_amount: gross, gst_treatment: 'standard', ...business_account })),
);
const office26 = [91_200, 77_640, 83_150];
const homeInternet26 = { gross: 10_499, share: 35 };
const expenses26 = [
  ...m26.map((m) => ({ category: 'Bank fees', supplier: 'Bank', expense_date: `${m}-28`, gross_amount: 790, gst_treatment: 'exempt', ...business_account })),
  ...['2025-06-30', '2025-09-30', '2025-12-31', '2026-03-31'].map((d) => ({ category: 'Interest expense', supplier: 'Bank', expense_date: d, gross_amount: 540, gst_treatment: 'exempt', ...business_account })),
  { category: 'Consulting & Accounting', supplier: 'Accountant', expense_date: '2025-08-20', gross_amount: 189_750, gst_treatment: 'standard', ...business_account },
  ...['2025-05-20', '2025-10-14', '2026-02-09'].map((d, i) => ({ category: 'Office expenses', supplier: 'Office costs', expense_date: d, gross_amount: office26[i], gst_treatment: 'standard', ...business_account })),
  ...subs,
  // A dedicated workspace from April 2025: 35% of the household internet is for the business.
  ...m26.map((m) => ({ category: 'Home Office- Internet', supplier: 'Internet provider', expense_date: `${m}-10`, gross_amount: homeInternet26.gross, gst_treatment: 'standard', payment_method: 'personal_funds', classification: 'partial', business_percentage: String(homeInternet26.share) })),
  { category: 'Non deductible penalties', supplier: 'Inland Revenue', expense_date: '2025-11-30', gross_amount: 36_400, gst_treatment: 'exempt', ...business_account },
];
const movements26 = [
  ...m26.map((m) => ({ kind: 'drawing', movement_date: `${m}-25`, amount: 1_020_000, description: 'Transfer to personal account' })),
  { kind: 'funds_introduced', movement_date: '2025-07-03', amount: 1_200_000, description: 'Owner contribution' },
  { kind: 'funds_introduced', movement_date: '2026-01-19', amount: 480_000, description: 'Owner contribution' },
  { kind: 'gst_payment', movement_date: '2025-05-07', amount: y25.gst, description: 'GST return, February – March 2025' },
  ...['2025-06-28', '2025-08-28', '2025-10-28', '2026-01-15', '2026-02-28'].map((d) => ({ kind: 'gst_payment', movement_date: d, amount: 310_000, description: 'GST return' })),
];
const dep26 = depreciate(assets, Object.fromEntries(dep25.map((a) => [a.name, a.closing])));
const y26 = year({ bank: y25.bank, gst_payable: y25.gst }, invoices26, expenses26, movements26, dep26, 874_300);
const ppe26 = dep26.reduce((s, a) => s + a.closing, 0);
const closing26 = y26.bank + ppe26 - y26.gst;

// ─── The reports' figures ───────────────────────────────────────────────────

const pnl = (y) => ({
  Sales: y.sales,
  'Total income': y.sales,
  'Gross profit': y.sales,
  ...y.expenses,
  'Total expenses': y.totalExpenses,
  'Net profit before adjustments': y.profitBeforeAdjustments,
  ...y.adjustments,
  'Total non-deductible expenses': Object.values(y.adjustments).reduce((a, b) => a + b, 0),
  'Net profit': y.profit,
});
const report = {
  2025: {
    pnl: pnl(y25),
    sheet: {
      'Business bank account': y25.bank,
      'Property, plant and equipment': ppe25,
      'Total assets': y25.bank + ppe25,
      'GST payable': y25.gst,
      'Net assets': closing25,
      'Current year earnings': y25.profit,
      Drawings: equity25.drawings,
      'Owner funds introduced': equity25.funds,
      'Personal accounts': opening2024.personal_accounts,
      'Retained earnings': retained24,
      'Withholding tax paid': equity25.withholding,
      'Total equity': closing25,
    },
    equity: {
      'Opening balance': openingEquity25,
      'Profit for the period': y25.profit,
      'Owner funds introduced': y25.equity.funds,
      'Total increases': y25.profit + y25.equity.funds,
      Drawings: -y25.equity.drawings,
      'Withholding tax paid': -y25.equity.withholding,
      'Total decreases': -y25.equity.drawings - y25.equity.withholding,
      'Total equity': closing25,
    },
  },
  2026: {
    pnl: pnl(y26),
    sheet: {
      'Business bank account': y26.bank,
      'Property, plant and equipment': ppe26,
      'Total assets': y26.bank + ppe26,
      'GST payable': y26.gst,
      'Net assets': closing26,
      'Current year earnings': y26.profit,
      Drawings: equity25.drawings + y26.equity.drawings,
      'Owner funds introduced': equity25.funds + y26.equity.funds,
      'Personal accounts': opening2024.personal_accounts,
      'Retained earnings': retained24 + y25.profit,
      'Withholding tax paid': equity25.withholding + y26.equity.withholding,
      'Total equity': closing26,
    },
    equity: {
      'Opening balance': closing25,
      'Profit for the period': y26.profit,
      'Owner funds introduced': y26.equity.funds,
      'Total increases': y26.profit + y26.equity.funds,
      Drawings: -y26.equity.drawings,
      'Withholding tax paid': -y26.equity.withholding,
      'Total decreases': -y26.equity.drawings - y26.equity.withholding,
      'Total equity': closing26,
    },
  },
};

// What an accountant's annual report would print: each line in whole dollars. The integration tests
// check that Solo, through its API, arrives at the same figures as this independent simulation.
const toDollars = (part) => Object.fromEntries(Object.entries(part).map(([label, cents]) => [label, dollars(cents)]));
const printed = Object.fromEntries(
  Object.entries(report).map(([fy, parts]) => [fy, Object.fromEntries(Object.entries(parts).map(([part, figures]) => [part, toDollars(figures)]))]),
);
for (const fy of ['2025', '2026']) {
  for (const part of ['pnl', 'sheet', 'equity']) {
    for (const [label, value] of Object.entries(printed[fy][part])) check(`FY${fy} ${part} ${label}`, report[fy][part][label], value);
  }
}
// The balance sheet balances to the cent, and the changes in equity reconcile to it.
for (const [fy, closing] of [['2025', closing25], ['2026', closing26]]) {
  const s = report[fy].sheet;
  const equity = s['Current year earnings'] + s.Drawings + s['Owner funds introduced'] + s['Personal accounts'] + s['Retained earnings'] + s['Withholding tax paid'];
  if (equity !== closing) throw new Error(`FY${fy}: equity ${equity} ≠ net assets ${closing}`);
}

// ─── Fixtures ───────────────────────────────────────────────────────────────

const scheduleExpected = (dep) => ({
  ...Object.fromEntries(dep.map((a) => [a.name, { cost: dollars(a.cost), opening: dollars(a.opening), depreciation: dollars(a.depreciation), closing: dollars(a.closing) }])),
  'Total computer equipment': {
    cost: dollars(dep.reduce((s, a) => s + a.cost, 0)),
    opening: dollars(dep.reduce((s, a) => s + a.opening, 0)),
    depreciation: dollars(dep.reduce((s, a) => s + a.depreciation, 0)),
    closing: dollars(dep.reduce((s, a) => s + a.closing, 0)),
  },
});
const about = (fy) => [
  `Fernleaf Consulting, a made-up sole trader, for the year ended 31 March ${fy}. Every name and amount is invented.`,
  'build.mjs generates this file: it simulates Solo’s posting rules on these records independently of the API and gives each report line in whole dollars, as an accountant’s annual report prints it.',
];
const categories = [
  { name: 'Interest expense', non_deductible: false },
  { name: 'Office expenses', non_deductible: false },
  { name: 'Home Office- Internet', non_deductible: false },
  { name: 'Non deductible penalties', non_deductible: true },
];

const fy2025 = {
  about: about(2025),
  business,
  year: { from: '2024-04-01', to: '2025-03-31' },
  opening_balances: opening2024,
  assets,
  categories,
  customer: { name: 'Contract client' },
  invoices: invoices25,
  expenses: expenses25,
  movements: movements25,
  expected: {
    note: 'Each report line in whole dollars.',
    profit_and_loss: printed[2025].pnl,
    balance_sheet: printed[2025].sheet,
    balance_sheet_2024: {
      'Business bank account': dollars(opening2024.bank),
      'Property, plant and equipment': dollars(openingPpe),
      'Total assets': dollars(opening2024.bank + openingPpe),
      'GST payable': dollars(opening2024.gst_payable),
      'Net assets': dollars(openingEquity25),
      Drawings: dollars(opening2024.drawings),
      'Owner funds introduced': dollars(opening2024.funds_introduced),
      'Personal accounts': dollars(opening2024.personal_accounts),
      'Withholding tax paid': dollars(opening2024.withholding_tax),
      'Total equity': dollars(openingEquity25),
      retained_and_current: dollars(retained24),
    },
    equity_changes: printed[2025].equity,
    depreciation: scheduleExpected(dep25),
    checks: {
      book_value: dollars(ppe25),
      opening_retained_earnings: dollars(retained24),
      opening_fixed_assets: dollars(openingPpe),
      opening_fixed_assets_cents: openingPpe,
      depreciation_cents: dep25.reduce((s, a) => s + a.depreciation, 0),
      net_assets_csv: (closing25 / 100).toFixed(2),
    },
  },
};
const pair = (fy, part) => Object.fromEntries(Object.entries(printed[fy][part]).map(([k, v]) => [k, [v, printed[fy - 1]?.[part]?.[k] ?? 0]]));
const fy2026 = {
  about: [...about(2026), 'Entered after fy2025.json in the same business: the opening values come from the year before.'],
  year: { from: '2025-04-01', to: '2026-03-31' },
  categories: [
    { name: 'Consulting & Accounting', non_deductible: false },
    { name: 'Subscriptions', non_deductible: false },
  ],
  invoices: invoices26,
  expenses: expenses26,
  movements: movements26,
  expected: {
    note: 'Each report line in whole dollars: [this year, the comparative year], side by side.',
    profit_and_loss: pair(2026, 'pnl'),
    balance_sheet: pair(2026, 'sheet'),
    equity_changes: pair(2026, 'equity'),
    depreciation: scheduleExpected(dep26),
    checks: {
      book_value: dollars(ppe26),
      retained_earnings: dollars(retained24 + y25.profit),
      current_year_earnings: dollars(y26.profit),
      home_office_internet_cents: y26.expenses['Home Office- Internet'],
      home_office_internet: { monthly_gross: homeInternet26.gross, business_percentage: homeInternet26.share },
      owner_funds_introduced_cents: y26.equity.funds,
      drawings_cents: -y26.equity.drawings,
    },
  },
};

writeFileSync(new URL('fy2025.json', here), `${JSON.stringify(fy2025, null, 2)}\n`);
writeFileSync(new URL('fy2026.json', here), `${JSON.stringify(fy2026, null, 2)}\n`);
console.log('Fixtures written. FY2025 closing equity', closing25 / 100, '· FY2026', closing26 / 100);
console.log('Bank', y25.bank / 100, '→', y26.bank / 100, '· GST payable', y25.gst / 100, '→', y26.gst / 100);
