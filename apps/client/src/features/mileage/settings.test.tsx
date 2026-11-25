import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { MileageRate, Vehicle } from './api';
import { MileageSettings } from './MileageSettings';
import {
  defaultRateName,
  newRateSchema,
  newVersionValues,
  parseRatePerKm,
  rateInputValue,
  rateOptionLabel,
  toNewRateInput,
  toRateVersionInput,
} from './rateForm';
import { VehicleFormModal } from './VehicleFormModal';
import { emptyVehicleValues, fromVehicle, makeDefaultInput, toVehicleInput, vehicleSchema, type VehicleFormValues } from './vehicleForm';

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

const rate: MileageRate = {
  id: 'r1',
  name: 'Kilometre rate',
  notes: 'From the business’s records',
  archived: false,
  vehicle_count: 1,
  trip_count: 12,
  versions: [
    { id: 'rv1', effective_from: '2025-04-01', rate_per_km: '0.9500' },
    { id: 'rv2', effective_from: '2026-04-01', rate_per_km: '1.0250' },
  ],
  current: { id: 'rv2', effective_from: '2026-04-01', rate_per_km: '1.0250' },
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const vehicle: Vehicle = {
  id: 'v1',
  name: 'Work ute',
  registration: 'ABC123',
  make: 'Toyota',
  model: 'Hilux',
  year: 2019,
  vehicle_type: 'ute',
  fuel_type: 'diesel',
  default_rate_id: 'r1',
  default_rate_name: 'Kilometre rate',
  is_default: true,
  archived: false,
  trip_count: 12,
  notes: '',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

interface Sent {
  method: string;
  url: string;
  body: unknown;
}

/**
 * Answers the vehicle and rate lists, and every write with `write` (or the first vehicle); records
 * what was sent.
 */
function mockApi({
  vehicles = [],
  rates = [],
  write,
}: {
  vehicles?: Vehicle[];
  rates?: MileageRate[];
  write?: (request: Sent) => Response;
} = {}) {
  const sent: Sent[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    const url = new URL(request.url).pathname;
    if (request.method === 'GET') {
      if (url.endsWith('/mileage/vehicles')) return Response.json(vehicles);
      if (url.endsWith('/mileage/rates')) return Response.json(rates);
      return Response.json({});
    }
    const text = await request.clone().text();
    const entry = { method: request.method, url, body: text ? (JSON.parse(text) as unknown) : null };
    sent.push(entry);
    return write ? write(entry) : Response.json({ ...vehicle, ...(entry.body as object) });
  });
  return sent;
}

describe('rate form helpers', () => {
  it('parses a rate per km without floating point, between 0 and 100 with up to 4 decimals', () => {
    expect(parseRatePerKm('1.04')).toBe('1.04');
    expect(parseRatePerKm(' $0.8250 ')).toBe('0.825');
    expect(parseRatePerKm('.5')).toBe('0.5');
    expect(parseRatePerKm('2.')).toBe('2');
    expect(parseRatePerKm('0')).toBe('0');
    expect(parseRatePerKm('100.0000')).toBe('100');
    expect(parseRatePerKm('100.01')).toBeNull();
    expect(parseRatePerKm('1.04567')).toBeNull();
    expect(parseRatePerKm('-1')).toBeNull();
    expect(parseRatePerKm('1,04')).toBeNull();
    expect(parseRatePerKm('')).toBeNull();
    expect(parseRatePerKm('.')).toBeNull();
  });

  it('shows API rates in inputs without padding zeros', () => {
    expect(rateInputValue('1.0400')).toBe('1.04');
    expect(rateInputValue('0.8250')).toBe('0.825');
    expect(rateInputValue('1.0000')).toBe('1.00');
  });

  it('labels a rate by the version in force, else its first one', () => {
    expect(rateOptionLabel(rate, 'NZD')).toBe('Kilometre rate · $1.025/km from 01/04/2026');
    const future = { ...rate, current: null, versions: [{ id: 'rv3', effective_from: '2027-04-01', rate_per_km: '1.1000' }] };
    expect(rateOptionLabel(future, 'NZD')).toBe('Kilometre rate · $1.10/km from 01/04/2027');
    expect(rateOptionLabel({ ...rate, archived: true }, 'NZD')).toMatch(/\(archived\)$/);
  });

  it('suggests a name only when no active rate has it', () => {
    expect(defaultRateName([])).toBe('Kilometre rate');
    expect(defaultRateName([rate])).toBe('');
    expect(defaultRateName([{ ...rate, archived: true }])).toBe('Kilometre rate');
  });

  it('starts a new version on a free date: the financial year, else today', () => {
    expect(newVersionValues({ ...rate, versions: [] }, '2026-04-01', '2026-11-20').effective_from).toBe('2026-04-01');
    expect(newVersionValues(rate, '2026-04-01', '2026-11-20').effective_from).toBe('2026-11-20');
  });

  it('validates and converts with the API’s field names', () => {
    const empty = newRateSchema.safeParse({ name: ' ', notes: '', effective_from: null, rate_per_km: '' });
    expect(empty.error?.issues.map((i) => i.path.join('.'))).toEqual(['name', 'effective_from', 'rate_per_km']);
    expect(newRateSchema.safeParse({ name: 'Rate', notes: '', effective_from: '2026-04-01', rate_per_km: '150' }).success).toBe(false);
    expect(toNewRateInput({ name: ' Rate ', notes: ' ', effective_from: '2026-04-01', rate_per_km: '1.0400' })).toEqual({
      name: 'Rate',
      notes: '',
      effective_from: '2026-04-01',
      rate_per_km: '1.04',
    });
    expect(toRateVersionInput({ effective_from: '2026-04-01', rate_per_km: '$0.9' })).toEqual({
      effective_from: '2026-04-01',
      rate_per_km: '0.9',
    });
  });
});

describe('vehicle form helpers', () => {
  const values = (overrides: Partial<VehicleFormValues> = {}): VehicleFormValues => ({
    ...emptyVehicleValues({ rates: [], newRateFrom: '2026-04-01', firstVehicle: true }),
    ...overrides,
  });

  it('starts in new-rate mode without active rates, and with the only active rate otherwise', () => {
    expect(values()).toMatchObject({
      create_rate: true,
      default_rate_id: null,
      is_default: true,
      new_rate: { name: 'Kilometre rate', effective_from: '2026-04-01', rate_per_km: '' },
    });
    expect(emptyVehicleValues({ rates: [rate], newRateFrom: '2026-04-01', firstVehicle: false })).toMatchObject({
      create_rate: false,
      default_rate_id: 'r1',
      is_default: false,
      new_rate: { name: '' },
    });
    expect(emptyVehicleValues({ rates: [{ ...rate, archived: true }], newRateFrom: null, firstVehicle: false }).create_rate).toBe(true);
  });

  it('checks the new rate only when creating one, under new_rate.*', () => {
    const issues = (v: VehicleFormValues) => vehicleSchema.safeParse(v).error?.issues.map((i) => i.path.join('.')) ?? [];
    expect(issues(values())).toEqual(['name', 'new_rate.rate_per_km']);
    expect(issues(values({ name: 'Ute', year: 1800, new_rate: { name: '', notes: '', effective_from: null, rate_per_km: '1' } }))).toEqual([
      'year',
      'new_rate.name',
      'new_rate.effective_from',
    ]);
    expect(issues(values({ name: 'Ute', create_rate: false }))).toEqual([]);
  });

  it('sends either the chosen rate or the new one', () => {
    const withNew = toVehicleInput(values({ name: ' Ute ', year: '', new_rate: { ...values().new_rate, rate_per_km: '1.04' } }));
    expect(withNew).toMatchObject({
      name: 'Ute',
      year: null,
      default_rate_id: null,
      new_rate: { name: 'Kilometre rate', rate_per_km: '1.04', effective_from: '2026-04-01', notes: '' },
    });
    const chosen = toVehicleInput(values({ name: 'Ute', year: 2019, create_rate: false, default_rate_id: 'r1' }));
    expect(chosen).toMatchObject({ year: 2019, default_rate_id: 'r1', new_rate: null });
  });

  it('round-trips a vehicle and can make it the default', () => {
    const other = { ...vehicle, is_default: false };
    expect(toVehicleInput(fromVehicle(other))).toEqual({
      name: 'Work ute',
      registration: 'ABC123',
      make: 'Toyota',
      model: 'Hilux',
      year: 2019,
      vehicle_type: 'ute',
      fuel_type: 'diesel',
      default_rate_id: 'r1',
      new_rate: null,
      is_default: false,
      notes: '',
    });
    expect(makeDefaultInput(other)).toMatchObject({ name: 'Work ute', default_rate_id: 'r1', is_default: true });
  });
});

describe('VehicleFormModal', () => {
  it('requires a name before saving', async () => {
    const sent = mockApi({ rates: [rate] });
    renderWithProviders(<VehicleFormModal opened onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole('button', { name: 'Add vehicle' }));
    expect(await screen.findByText('Enter a name for the vehicle.')).toBeInTheDocument();
    expect(sent).toEqual([]);
  });

  it('offers the active rates, labelled with the rate in force', async () => {
    mockApi({ vehicles: [vehicle], rates: [rate, { ...rate, id: 'r2', name: 'Old rate', archived: true }] });
    renderWithProviders(<VehicleFormModal opened onClose={() => undefined} />);
    expect(await screen.findByDisplayValue('Kilometre rate · $1.025/km from 01/04/2026')).toBeInTheDocument();
    expect(screen.queryByLabelText(/^Rate name/)).not.toBeInTheDocument();
    expect(screen.getByRole('switch', { name: /^Use for new trips/ })).not.toBeChecked();
  });

  it('creates the rate with the first vehicle when there are no rates', async () => {
    const sent = mockApi();
    const onSaved = vi.fn();
    renderWithProviders(<VehicleFormModal opened onClose={() => undefined} onSaved={onSaved} />);
    await userEvent.type(await screen.findByLabelText(/^Name/), 'Work ute');
    expect(screen.getByLabelText(/^Rate name/)).toHaveValue('Kilometre rate');
    expect(screen.getByText(/doesn’t supply them/)).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: /^Use for new trips/ })).toBeChecked();
    await userEvent.click(screen.getByRole('button', { name: 'Add vehicle' }));
    expect(await screen.findByText('Enter the rate per kilometre.')).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText(/^Rate per km/), '1.04');
    await userEvent.click(screen.getByRole('button', { name: 'Add vehicle' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(sent).toEqual([
      {
        method: 'POST',
        url: '/api/v1/businesses/b1/mileage/vehicles',
        body: expect.objectContaining({
          name: 'Work ute',
          vehicle_type: 'car',
          fuel_type: 'petrol',
          default_rate_id: null,
          new_rate: { name: 'Kilometre rate', notes: '', effective_from: '2026-04-01', rate_per_km: '1.04' },
          is_default: true,
        }),
      },
    ]);
  });

  it('puts the API’s new_rate errors on the new-rate fields', async () => {
    mockApi({
      write: () =>
        Response.json(
          {
            error: {
              code: 'VALIDATION',
              message: 'Check the highlighted fields.',
              fields: { 'new_rate.name': 'A rate with this name already exists.' },
            },
          },
          { status: 422 },
        ),
    });
    renderWithProviders(<VehicleFormModal opened onClose={() => undefined} />);
    await userEvent.type(await screen.findByLabelText(/^Name/), 'Work ute');
    await userEvent.type(screen.getByLabelText(/^Rate per km/), '1.04');
    await userEvent.click(screen.getByRole('button', { name: 'Add vehicle' }));
    expect(await screen.findByText('A rate with this name already exists.')).toBeInTheDocument();
    expect(screen.getByLabelText(/^Rate name/)).toHaveAttribute('aria-invalid', 'true');
  });
});

describe('MileageSettings', () => {
  const archivedVehicle: Vehicle = { ...vehicle, id: 'v2', name: 'Old car', is_default: false, archived: true, trip_count: 0 };

  it('lists vehicles and each rate’s versions', async () => {
    mockApi({ vehicles: [vehicle, archivedVehicle], rates: [rate] });
    renderWithProviders(<MileageSettings />);
    expect(await screen.findByText('Work ute')).toBeInTheDocument();
    expect(screen.getByText('Toyota Hilux 2019 · ABC123 · Ute · Diesel')).toBeInTheDocument();
    expect(screen.getByText('Rate: Kilometre rate · 12 trips')).toBeInTheDocument();
    expect(screen.getByText('Default')).toBeInTheDocument();
    expect(screen.queryByText('Old car')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Show archived vehicles (1)' }));
    expect(screen.getByText('Old car')).toBeInTheDocument();

    expect(await screen.findByText('$1.025/km today · default for 1 vehicle · 12 trips')).toBeInTheDocument();
    const versions = screen.getByRole('table', { name: 'Kilometre rate versions' });
    const rows = within(versions).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringMatching(/^01\/04\/2026Current\$1\.025\/km/),
      expect.stringMatching(/^01\/04\/2025\$0\.95\/km/),
    ]);
    expect(screen.getByText(/A trip uses the version in force on its date and keeps it/)).toBeInTheDocument();
  });

  it('deletes a version after confirming', async () => {
    const sent = mockApi({ vehicles: [vehicle], rates: [rate], write: () => Response.json(rate) });
    renderWithProviders(<MileageSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Delete the rate from 01/04/2025' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Delete version' }));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ method: 'DELETE', url: '/api/v1/businesses/b1/mileage/rates/r1/versions/rv1' });
  });

  it('shows why a rate can’t be archived', async () => {
    mockApi({
      vehicles: [vehicle],
      rates: [rate],
      write: () =>
        Response.json(
          { error: { code: 'CONFLICT', message: 'Vehicles use this rate by default. Choose another rate for them first.' } },
          { status: 409 },
        ),
    });
    renderWithProviders(<MileageSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Archive Kilometre rate' }));
    expect(await screen.findByText('Vehicles use this rate by default. Choose another rate for them first.')).toBeInTheDocument();
  });

  it('offers to add the first vehicle and rate', async () => {
    mockApi();
    renderWithProviders(<MileageSettings />);
    expect(await screen.findByText('No vehicles yet')).toBeInTheDocument();
    expect(await screen.findByText('No mileage rates yet')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Add vehicle' }));
    expect(await screen.findByLabelText(/^Rate name/)).toHaveValue('Kilometre rate');
  });
});
