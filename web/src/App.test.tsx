import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { App } from './App';

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => body,
  } as Response;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.location.hash = '#/';
});

describe('App', () => {
  it('renders repository names after the list request succeeds', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse([
        {
          id: 'repo-1',
          name: 'alpha-repo',
          default_branch: 'main',
          sync_state: 'ready',
        },
      ])
    );

    render(<App />);

    expect(await screen.findByText('alpha-repo')).toBeInTheDocument();
    expect(screen.getByText('Default branch: main')).toBeInTheDocument();
  });

  it('loads repository detail and browses directories and files from the browse api', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
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
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [
            { name: 'src', path: 'src', kind: 'tree' },
            { name: 'README.md', path: 'README.md', kind: 'blob' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'App.tsx', path: 'src/App.tsx', kind: 'blob' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2FApp.tsx') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/App.tsx',
          size_bytes: 42,
          content: 'export function App() { return null; }',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(screen.getByText('Repository id: repo-42')).toBeInTheDocument();
    expect(screen.getByText('GitHub App')).toBeInTheDocument();
    const srcButton = (await screen.findByText('src/')).closest('button');
    const readmeButton = screen.getByText('README.md').closest('button');

    expect(srcButton).toBeInTheDocument();
    expect(readmeButton).toBeInTheDocument();

    fireEvent.click(srcButton!);

    expect(await screen.findByText('Current path: src')).toBeInTheDocument();
    const appFileButton = screen.getByText('App.tsx').closest('button');
    expect(appFileButton).toBeInTheDocument();

    fireEvent.click(appFileButton!);

    expect(await screen.findByText('src/App.tsx')).toBeInTheDocument();
    expect(screen.getByText('42 bytes')).toBeInTheDocument();
    expect(screen.getByText('export function App() { return null; }')).toBeInTheDocument();

    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=src');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2FApp.tsx');
  });

  it('shows browse errors without breaking repository detail rendering', async () => {
    window.location.hash = '#/repos/repo-42';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
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
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({}, false, 503);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(await screen.findByText('Unable to load files: Request failed: 503')).toBeInTheDocument();
  });

  it('shows an error state when the repository list request fails', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({}, false, 500));

    render(<App />);

    expect(await screen.findByText('Failed to load: Request failed: 500')).toBeInTheDocument();
  });
});
