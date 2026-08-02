import { afterEach, describe, expect, test, vi } from 'vitest';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';

import { SUBSCRIPTION_CONNECT_PROVIDERS, SubscriptionConnectModal } from './SubscriptionConnectModal.jsx';

afterEach(cleanup);

describe('SubscriptionConnectModal', () => {
  test('renders the Codex device flow without exposing provider credentials', () => {
    render(
      <SubscriptionConnectModal
        initial={{
          state: 'pending',
          connected: false,
          verification_uri: 'https://auth.example/device',
          user_code: 'ABCD-EFGH',
        }}
        subscription={SUBSCRIPTION_CONNECT_PROVIDERS.codex}
        subjectLabel="this Codex agent"
        getStatus={vi.fn().mockResolvedValue(null)}
        onConnected={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole('dialog', { name: 'Connect ChatGPT subscription' })).toBeInTheDocument();
    expect(screen.getByText(/directly from this Codex agent/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /Open sign-in/ })).toHaveAttribute('href', 'https://auth.example/device');
    expect(screen.getByRole('button', { name: 'Copy device code' })).toHaveTextContent('ABCD-EFGH');
    expect(screen.queryByText(/access.token|refresh.token/i)).toBeNull();
  });

  test('submits Claude returned codes through the supplied broker adapter', async () => {
    const completeAuth = vi.fn().mockResolvedValue({ state: 'connected', connected: true });
    const onConnected = vi.fn();
    render(
      <SubscriptionConnectModal
        initial={{ state: 'pending', connected: false, auth_url: 'https://auth.example/claude' }}
        subscription={SUBSCRIPTION_CONNECT_PROVIDERS.claude}
        getStatus={vi.fn().mockResolvedValue(null)}
        completeAuth={completeAuth}
        onConnected={onConnected}
        onClose={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText('Authorization code'), { target: { value: 'code#state' } });
    fireEvent.click(screen.getByRole('button', { name: 'Connect' }));

    await waitFor(() => expect(completeAuth).toHaveBeenCalledWith('code#state'));
    expect(onConnected).toHaveBeenCalledTimes(1);
  });
});
