import { expect, test } from '@playwright/test';
import { login } from './helpers';

// Smoke test of the SNMP tool (ADR-0010): the screen opens, its three tabs switch,
// the request form validates before anything is sent, and the MIB tab loads.
test('SNMP tool: tabs, form validation and the MIB list', async ({ page }) => {
  await login(page);
  await page.getByRole('link', { name: 'SNMP' }).first().click();
  await expect(page.getByRole('heading', { level: 1, name: 'SNMP' })).toBeVisible();

  const tabs = page.getByRole('tablist', { name: 'SNMP' });
  await expect(tabs.getByRole('tab', { name: 'Operations' })).toHaveAttribute('aria-selected', 'true');

  // Operations: an empty host is rejected on the client.
  await expect(page.getByRole('textbox', { name: 'Host' })).toBeVisible();
  const oids = page.getByRole('combobox', { name: 'OIDs / object names' });
  await oids.fill('sysDescr.0');
  await page.getByRole('button', { name: 'Get', exact: true }).click();
  await expect(page.getByText('Enter the host.').first()).toBeVisible();

  // Traps: nothing has been received yet.
  await tabs.getByRole('tab', { name: 'Traps' }).click();
  await expect(tabs.getByRole('tab', { name: 'Traps' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByText('Nothing received yet.')).toBeVisible();

  // MIBs: the module list is shown.
  await tabs.getByRole('tab', { name: 'MIBs' }).click();
  await expect(tabs.getByRole('tab', { name: 'MIBs' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByText('MIB modules').first()).toBeVisible();
});
