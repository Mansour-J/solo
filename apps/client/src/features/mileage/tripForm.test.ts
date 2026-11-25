import type { FrequentTrip, Trip, Vehicle } from './api';
import { formatKm, formatRate, formatShare, vehicleDescription } from './format';
import {
  calculationInput,
  CUSTOM_RATE,
  defaultVehicle,
  emptyTrip,
  frequentLabel,
  fromFrequent,
  fromTrip,
  toTripInput,
  tripSchema,
  type TripFormValues,
} from './tripForm';

const base: TripFormValues = { ...emptyTrip('2026-11-20', 'v1'), purpose: ' Client visit ', distance_km: '42.5' };

function paths(values: TripFormValues): string[] {
  return tripSchema.safeParse(values).error?.issues.map((i) => i.path.join('.')) ?? [];
}

describe('trip form', () => {
  it('starts with the default vehicle and the full trip for the business', () => {
    const vehicles = [
      { id: 'a', archived: false, is_default: false },
      { id: 'b', archived: false, is_default: true },
      { id: 'c', archived: true, is_default: false },
    ] as Vehicle[];
    expect(defaultVehicle(vehicles)?.id).toBe('b');
    expect(defaultVehicle([vehicles[0]!, vehicles[2]!])?.id).toBe('a');
    expect(defaultVehicle([vehicles[2]!])).toBeUndefined();
    expect(emptyTrip('2026-11-20', 'b')).toMatchObject({ trip_date: '2026-11-20', business_percentage: '100', distance_mode: 'distance' });
  });

  it('sends only the fields for the chosen distance and rate', () => {
    expect(toTripInput(base)).toMatchObject({
      purpose: 'Client visit',
      distance_km: '42.5',
      start_odometer: null,
      end_odometer: null,
      rate_id: null,
      rate_per_km: null,
    });
    const odometer = toTripInput({ ...base, distance_mode: 'odometer', start_odometer: '10,000', end_odometer: '10 042.5' });
    expect(odometer).toMatchObject({ start_odometer: '10000', end_odometer: '10042.5', distance_km: null });
    expect(toTripInput({ ...base, rate_id: 'rate-2' })).toMatchObject({ rate_id: 'rate-2', rate_per_km: null });
    expect(toTripInput({ ...base, rate_id: CUSTOM_RATE, rate_per_km: '0.85' })).toMatchObject({ rate_id: null, rate_per_km: '0.85' });
  });

  it('validates with the API’s field names', () => {
    expect(paths(base)).toEqual([]);
    expect(paths({ ...base, purpose: '', vehicle_id: null })).toEqual(expect.arrayContaining(['purpose', 'vehicle_id']));
    expect(paths({ ...base, distance_km: '0' })).toEqual(['distance_km']);
    expect(paths({ ...base, distance_km: '12.34' })).toEqual(['distance_km']);
    expect(paths({ ...base, distance_mode: 'odometer', start_odometer: '100', end_odometer: '90' })).toEqual(['end_odometer']);
    expect(paths({ ...base, business_percentage: '101' })).toEqual(['business_percentage']);
    expect(paths({ ...base, rate_id: CUSTOM_RATE, rate_per_km: 'one dollar' })).toEqual(['rate_per_km']);
  });

  it('asks the API to calculate only once the trip is complete enough', () => {
    expect(calculationInput({ ...base, distance_km: '' })).toBeNull();
    expect(calculationInput({ ...base, business_percentage: '150' })).toBeNull();
    expect(calculationInput({ ...base, rate_id: CUSTOM_RATE, rate_per_km: '' })).toBeNull();
    expect(calculationInput({ ...base, purpose: '' })).toMatchObject({ distance_km: '42.5', purpose: '' });
  });

  it('edits a trip with the options it was recorded with', () => {
    const trip = {
      vehicle_id: 'v1',
      trip_date: '2026-05-01',
      start_location: 'Home',
      destination: 'Site',
      purpose: 'Inspection',
      start_odometer: '10000.0',
      end_odometer: '10042.5',
      distance_km: '42.5',
      business_percentage: '60.00',
      rate_id: null,
      rate_per_km: '0.8500',
      rate_overridden: true,
      notes: '',
    } as Trip;
    expect(fromTrip(trip)).toMatchObject({
      distance_mode: 'odometer',
      start_odometer: '10000',
      end_odometer: '10042.5',
      business_percentage: '60',
      rate_id: CUSTOM_RATE,
      rate_per_km: '0.85',
    });
    expect(fromTrip({ ...trip, start_odometer: null, end_odometer: null, rate_overridden: false, rate_id: 'r1' })).toMatchObject({
      distance_mode: 'distance',
      distance_km: '42.5',
      rate_id: 'r1',
      rate_per_km: '',
    });
  });
});

describe('mileage formatting', () => {
  it('formats kilometres, rates and shares for people', () => {
    expect(formatKm('1234.5')).toBe('1,234.5 km');
    expect(formatKm('0')).toBe('0.0 km');
    expect(formatKm(null)).toBe('—');
    expect(formatRate('1.0400')).toBe('$1.04/km');
    expect(formatRate('0.8250')).toBe('$0.825/km');
    expect(formatShare('62.50')).toBe('62.5%');
    expect(vehicleDescription({ make: 'Toyota', model: 'Corolla', year: 2019, registration: 'ABC123' })).toBe(
      'Toyota Corolla 2019 · ABC123',
    );
    expect(vehicleDescription({ make: '', model: '', year: null, registration: '' })).toBe('');
  });
});

describe('return and frequent trips', () => {
  const frequent: FrequentTrip = {
    vehicle_id: 'v1',
    vehicle_name: 'Car',
    start_location: 'Home',
    destination: 'Client office',
    purpose: 'Client site',
    distance_km: '20.0',
    return_trip: true,
    business_percentage: '100.00',
    trips: 4,
    last_date: '2026-05-11',
  };

  it('fills a new trip from a frequent one, keeping its date', () => {
    const values = fromFrequent(emptyTrip('2026-06-01', null), frequent);
    expect(values).toMatchObject({
      vehicle_id: 'v1',
      trip_date: '2026-06-01',
      distance_km: '20',
      return_trip: true,
      purpose: 'Client site',
    });
    expect(toTripInput(values)).toMatchObject({ distance_km: '20', return_trip: true });
    expect(frequentLabel(frequent)).toBe('Client site · Home → Client office · 20 km each way');
  });

  it('sends a return trip only with a distance, and edits one way', () => {
    const odometer = { ...emptyTrip('2026-06-01', 'v1'), distance_mode: 'odometer' as const, return_trip: true };
    expect(toTripInput(odometer).return_trip).toBe(false);
    const saved = { distance_km: '40.0', return_trip: true, start_odometer: null, end_odometer: null } as unknown as Trip;
    expect(fromTrip({ ...saved, business_percentage: '100.00', notes: '' } as Trip)).toMatchObject({
      distance_km: '20',
      return_trip: true,
    });
  });
});
