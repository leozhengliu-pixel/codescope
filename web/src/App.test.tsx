import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from './App';

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => body,
  } as Response;
}

function deferredResponse() {
  let resolve!: (response: Response) => void;
  const promise = new Promise<Response>((innerResolve) => {
    resolve = innerResolve;
  });

  return { promise, resolve };
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

    expect(await screen.findByText('Default branch: main')).toBeInTheDocument();
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
            { name: 'src', path: 'src', kind: 'dir' },
            { name: 'README.md', path: 'README.md', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'App.tsx', path: 'src/App.tsx', kind: 'file' }],
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

  it('loads recent commits and shows commit details when a commit is selected', async () => {
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
            sync_state: 'ready',
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
          entries: [],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'abc123def456',
              short_id: 'abc123d',
              summary: 'Add commit browser',
              author_name: 'Alice Example',
              authored_at: '2026-04-15T00:00:00Z',
            },
            {
              id: 'fedcba654321',
              short_id: 'fedcba6',
              summary: 'Tidy search UI',
              author_name: 'Bob Example',
              authored_at: '2026-04-14T12:30:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit: {
            id: 'abc123def456',
            short_id: 'abc123d',
            summary: 'Add commit browser',
            body: 'Adds a minimal recent commits panel.\n\nIncludes commit metadata.',
            author_name: 'Alice Example',
            authored_at: '2026-04-15T00:00:00Z',
            parents: ['parent-1', 'parent-2'],
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456/diff') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit_id: 'abc123def456abc123def456abc123def456abcd',
          files: [
            {
              path: 'web/src/App.tsx',
              change_type: 'modified',
              old_path: null,
              additions: 12,
              deletions: 3,
              patch: '@@ -1,3 +1,4 @@\n-import oldThing\n+import newThing',
            },
            {
              path: 'web/src/NewName.tsx',
              change_type: 'renamed',
              old_path: 'web/src/OldName.tsx',
              additions: 0,
              deletions: 0,
              patch: '@@ -1 +1 @@\n-export * from "./OldName"\n+export * from "./NewName"',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Recent commits')).toBeInTheDocument();
    expect(screen.getByText('Add commit browser')).toBeInTheDocument();
    expect(screen.getByText('Alice Example')).toBeInTheDocument();
    expect(screen.getByText('abc123d')).toBeInTheDocument();
    expect(screen.getByText('Select a commit to inspect its details.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Add commit browser/i }));

    expect(await screen.findByText('Commit details')).toBeInTheDocument();
    expect(screen.getByText('Adds a minimal recent commits panel. Includes commit metadata.')).toBeInTheDocument();
    expect(screen.getByText('2026-04-15T00:00:00Z')).toBeInTheDocument();
    expect(screen.getByText('parent-1')).toBeInTheDocument();
    expect(screen.getByText('parent-2')).toBeInTheDocument();
    expect(await screen.findByText('Changed files')).toBeInTheDocument();
    expect(screen.getByText('web/src/App.tsx')).toBeInTheDocument();
    expect(screen.getByText('modified')).toBeInTheDocument();
    expect(screen.getByText('+12')).toBeInTheDocument();
    expect(screen.getByText('-3')).toBeInTheDocument();
    expect(screen.getByText('web/src/OldName.tsx → web/src/NewName.tsx')).toBeInTheDocument();
    expect(screen.getByText(/@@ -1,3 \+1,4 @@/)).toBeInTheDocument();
    expect(screen.getByText(/\+import newThing/)).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits?limit=20');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits/abc123def456');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits/abc123def456/diff');
  });

  it('shows a friendly fallback when a changed file has no patch text', async () => {
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
            sync_state: 'ready',
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
          entries: [],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'abc123def456',
              short_id: 'abc123d',
              summary: 'Add binary asset',
              author_name: 'Alice Example',
              authored_at: '2026-04-15T00:00:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit: {
            id: 'abc123def456',
            short_id: 'abc123d',
            summary: 'Add binary asset',
            author_name: 'Alice Example',
            authored_at: '2026-04-15T00:00:00Z',
            parents: ['parent-1'],
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456/diff') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit_id: 'abc123def456abc123def456abc123def456abcd',
          files: [
            {
              path: 'assets/logo.png',
              change_type: 'added',
              old_path: null,
              additions: 0,
              deletions: 0,
              patch: null,
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Add binary asset')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Add binary asset/i }));

    expect(await screen.findByText('assets/logo.png')).toBeInTheDocument();
    expect(screen.getByText('Binary file or patch unavailable.')).toBeInTheDocument();
  });

  it('renders the authenticated connections inventory and creates a new github connection from the settings route', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Acme GitHub ',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.example.com',
            },
          })
        );

        return jsonResponse(
          {
            id: 'conn-3',
            name: 'Acme GitHub',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.example.com',
            },
          },
          true,
          201
        );
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Kind: github')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();
    expect(screen.getByText('Repo path: /srv/git/mirror')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: ' Acme GitHub ' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'github' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://github.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Acme GitHub')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.example.com')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Acme GitHub ',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.example.com',
          },
        }),
      });
    });
  });

  it('renders all supported authenticated connection kind options on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const kindSelect = screen.getByLabelText('Connection kind');
    const optionValues = Array.from(kindSelect.querySelectorAll('option')).map((option) => option.textContent);
    expect(optionValues).toEqual([
      'github',
      'gitlab',
      'gitea',
      'gerrit',
      'bitbucket',
      'azure_devops',
      'generic_git',
      'local',
    ]);
  });

  it('shows authenticated connection management controls only after the initial settings inventory finishes loading', async () => {
    window.location.hash = '#/settings/connections';

    let resolveConnections: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(
      (async (input, init) => {
        const url = String(input);

        if (url === '/api/v1/auth/connections' && !init) {
          return await new Promise<Response>((resolve) => {
            resolveConnections = resolve;
          });
        }

        throw new Error(`Unhandled fetch: ${url}`);
      }) as typeof fetch
    );

    render(<App />);

    expect(await screen.findByText('Loading connections…')).toBeInTheDocument();
    expect(screen.queryByLabelText('Connection name')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Connection kind')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Create connection' })).not.toBeInTheDocument();

    resolveConnections?.(jsonResponse([]));

    await waitFor(() => {
      expect(screen.queryByText('Loading connections…')).not.toBeInTheDocument();
    });
    expect(screen.getByLabelText('Connection name')).toBeInTheDocument();
    expect(screen.getByLabelText('Connection kind')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create connection' })).not.toBeDisabled();
  });

  it('shows authenticated connection creation failures on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: 'Denied connection' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'local' } });
    fireEvent.change(screen.getByLabelText('Repo path'), { target: { value: '/srv/git/denied' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Failed to create connection: Request failed: 403')).toBeInTheDocument();
    expect(screen.queryByText('Denied connection')).not.toBeInTheDocument();
  });

  it('hides authenticated connection management controls when the settings inventory cannot be loaded', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Failed to load connections: Request failed: 403')).toBeInTheDocument();
    expect(screen.queryByLabelText('Connection name')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Connection kind')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Create connection' })).not.toBeInTheDocument();
  });

  it('shows authenticated connection loading failures on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('Failed to load connections: Request failed: 403')).toBeInTheDocument();
  });

  it('finds definitions for the selected file symbol and renders ordered navigable candidates with revision metadata', async () => {
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
            sync_state: 'ready',
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
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'main.rs', path: 'src/main.rs', kind: 'file' },
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 28,
          content: 'fn helper() { lib::helper(); }',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/main.rs',
          revision: 'rev-def-123',
          symbol: 'helper',
          definitions: [
            {
              path: 'src/lib.rs',
              name: 'helper',
              kind: 'function',
              range: { start_line: 3, end_line: 6 },
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Flib.rs&revision=rev-def-123#L3',
            },
            {
              path: 'src/main.rs',
              name: 'helper',
              kind: 'function',
              range: { start_line: 10, end_line: 12 },
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs&revision=rev-def-123#L10',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);

    expect(await screen.findByText('fn helper() { lib::helper(); }')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));

    expect(await screen.findByText('Definition results')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-def-123')).toBeInTheDocument();
    expect(screen.getAllByText('helper').length).toBeGreaterThan(0);
    expect(screen.getByText('src/lib.rs')).toBeInTheDocument();
    expect(screen.getByText('Lines 3–6')).toBeInTheDocument();
    expect(screen.getAllByText('src/main.rs').length).toBeGreaterThan(0);
    expect(screen.getByText('Lines 10–12')).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper');
  });

  it('ignores stale navigation responses after the user switches files', async () => {
    window.location.hash = '#/repos/repo-42';

    const pendingDefinitions = deferredResponse();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
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
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'main.rs', path: 'src/main.rs', kind: 'file' },
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 20,
          content: 'fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Flib.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          size_bytes: 24,
          content: 'export const value = 42;',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper') {
        return pendingDefinitions.promise;
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);
    expect(await screen.findByText('fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));
    expect(await screen.findByText('Loading code navigation…')).toBeInTheDocument();

    fireEvent.click((await screen.findByText('lib.rs')).closest('button')!);
    expect(await screen.findByText('export const value = 42;')).toBeInTheDocument();
    expect(screen.queryByText('Definition results')).not.toBeInTheDocument();

    pendingDefinitions.resolve(
      jsonResponse({
        status: 'supported',
        repo_id: 'repo-42',
        path: 'src/main.rs',
        revision: 'rev-stale-123',
        symbol: 'helper',
        definitions: [
          {
            path: 'src/main.rs',
            name: 'helper',
            kind: 'function',
            range: { start_line: 1, end_line: 1 },
            browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs&revision=rev-stale-123#L1',
          },
        ],
      })
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading code navigation…')).not.toBeInTheDocument();
      expect(screen.queryByText('Definition results')).not.toBeInTheDocument();
      expect(screen.getByText('export const value = 42;')).toBeInTheDocument();
    });
  });

  it('finds references and opens the referenced file when a result is clicked', async () => {
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
            sync_state: 'ready',
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
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
            { name: 'consumer.rs', path: 'src/consumer.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Flib.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          size_bytes: 24,
          content: 'pub fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/consumer.rs',
          size_bytes: 37,
          content: 'fn run() { crate::helper(); helper(); }',
        });
      }

      if (url === '/api/v1/repos/repo-42/references?path=src%2Flib.rs&symbol=helper') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          revision: 'rev-ref-456',
          symbol: 'helper',
          references: [
            {
              path: 'src/consumer.rs',
              line_number: 8,
              line: 'fn run() { crate::helper(); helper(); }',
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456#L8',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('lib.rs')).closest('button')!);

    expect(await screen.findByText('pub fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find references' }));

    expect(await screen.findByText('Reference results')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-ref-456')).toBeInTheDocument();
    expect(screen.getByText('Line 8')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /src\/consumer\.rs/i }));

    expect(await screen.findByText('src/consumer.rs')).toBeInTheDocument();
    expect(screen.getAllByText('fn run() { crate::helper(); helper(); }').length).toBeGreaterThan(0);
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/references?path=src%2Flib.rs&symbol=helper');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456');
  });

  it('shows a non-fatal capability notice for unsupported navigation responses', async () => {
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
            sync_state: 'ready',
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
          entries: [{ name: 'notes.txt', path: 'notes.txt', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=notes.txt') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'notes.txt',
          size_bytes: 18,
          content: 'plain text helper',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=notes.txt&symbol=helper') {
        return jsonResponse({
          status: 'unsupported',
          repo_id: 'repo-42',
          path: 'notes.txt',
          revision: null,
          symbol: 'helper',
          capability: 'Definitions are not available for plain text files.',
          definitions: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('notes.txt')).closest('button')!);
    expect(await screen.findByText('plain text helper')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));

    expect(await screen.findByText('Definitions are not available for plain text files.')).toBeInTheDocument();
    expect(screen.getByText('plain text helper')).toBeInTheDocument();
  });

  it('shows a friendly no-results state when navigation returns an empty supported list', async () => {
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
            sync_state: 'ready',
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
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'main.rs', path: 'src/main.rs', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 16,
          content: 'fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/references?path=src%2Fmain.rs&symbol=missing') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/main.rs',
          revision: 'rev-empty-789',
          symbol: 'missing',
          references: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);
    expect(await screen.findByText('fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'missing' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find references' }));

    expect(await screen.findByText('No references found for “missing”.')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-empty-789')).toBeInTheDocument();
  });

  it('searches code and filters results by repository from the home page', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'develop',
            sync_state: 'pending',
          },
        ]);
      }

      if (url === '/api/v1/search?q=needle&repo_id=repo-2') {
        return jsonResponse({
          query: 'needle',
          repo_id: 'repo-2',
          results: [
            {
              repo_id: 'repo-2',
              path: 'src/search.ts',
              line_number: 12,
              line: 'const needle = true;',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Default branch: main')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'needle' } });
    fireEvent.change(screen.getByLabelText('Repository filter'), { target: { value: 'repo-2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('src/search.ts')).toBeInTheDocument();
    expect(screen.getAllByText('beta-repo').length).toBeGreaterThan(0);
    expect(screen.getByText('Line 12')).toBeInTheDocument();
    expect(screen.getByText('const needle = true;')).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/search?q=needle&repo_id=repo-2');
  });

  it('shows an empty search state when no matches are returned', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/search?q=missing&repo_id=') {
        return jsonResponse({
          query: 'missing',
          repo_id: null,
          results: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Default branch: main')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'missing' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('No matches found for “missing”.')).toBeInTheDocument();
  });

  it('shows an error state when the repository list request fails', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({}, false, 500));

    render(<App />);

    expect(await screen.findByText('Failed to load: Request failed: 500')).toBeInTheDocument();
  });
});
