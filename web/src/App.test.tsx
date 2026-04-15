import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { App } from './App';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.location.hash = '#/';
});

describe('App', () => {
  it('renders repository names after the list request succeeds', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      json: async () => [
        {
          id: 'repo-1',
          name: 'alpha-repo',
          default_branch: 'main',
          sync_state: 'ready',
        },
      ],
    } as Response);

    render(<App />);

    expect(await screen.findByText('alpha-repo')).toBeInTheDocument();
    expect(screen.getByText('Default branch: main')).toBeInTheDocument();
  });

  it('loads and displays repository detail on the detail route', async () => {
    window.location.hash = '#/repos/repo-42';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        repository: {
          id: 'repo-42',
          name: 'beta-repo',
          default_branch: 'develop',
          connection_id: 'conn-7',
          sync_state: 'pending',
        },
        connection: {
          id: 'conn-7',
          name: 'GitHub App',
          kind: 'github',
        },
      }),
    } as Response);

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(screen.getByText('Repository id: repo-42')).toBeInTheDocument();
    expect(screen.getByText('GitHub App')).toBeInTheDocument();
  });

  it('shows an error state when a request fails', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: false,
      status: 500,
      json: async () => ({})
    } as Response);

    render(<App />);

    expect(await screen.findByText('Failed to load: Request failed: 500')).toBeInTheDocument();
  });
});
