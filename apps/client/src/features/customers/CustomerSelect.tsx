import { Select, type SelectProps } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { useState } from 'react';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useCustomer, useCustomers } from './api';

/** Searchable customer picker (active customers; keeps showing the selected one even if inactive). */
export function CustomerSelect({
  value,
  onChange,
  ...props
}: Omit<SelectProps, 'data' | 'value' | 'onChange'> & { value: string | null; onChange: (id: string | null) => void }) {
  const business = useCurrentBusiness();
  const [search, setSearch] = useState('');
  const [debounced] = useDebouncedValue(search, 200);
  const list = useCustomers(business.id, { search: debounced || undefined, status: 'active', per_page: 50 });
  const selected = useCustomer(business.id, value);
  const options = new Map<string, string>();
  for (const c of list.data?.items ?? []) options.set(c.id, c.name);
  if (selected.data) options.set(selected.data.id, selected.data.name);
  return (
    <Select
      searchable
      clearable={false}
      nothingFoundMessage={list.isFetching ? 'Searching…' : 'No customers found'}
      placeholder="Choose a customer"
      data={[...options].map(([id, name]) => ({ value: id, label: name }))}
      value={value}
      onChange={onChange}
      searchValue={search}
      onSearchChange={setSearch}
      filter={({ options: all }) => all}
      {...props}
    />
  );
}
