import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
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
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
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

  it('renders the authenticated connections inventory and creates a new generic git connection from the settings route using the host-style base_url contract', async () => {
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
            name: ' Generic Git Host ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse(
          {
            id: 'conn-3',
            name: 'Generic Git Host',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
          true,
          201
        );
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();
    expect(screen.getByText('Repo path: /srv/git/mirror')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: ' Generic Git Host ' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'generic_git' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://git.internal.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Generic Git Host')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: generic_git')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-3')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Generic Git Host ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
  });

  it('shows read-only repository sync-job history on each authenticated connection card from the settings route', async () => {
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-1',
            organization_id: 'org-1',
            repository_id: 'repo-123',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:02:00Z',
            error: null,
          },
          {
            id: 'job-2',
            organization_id: 'org-1',
            repository_id: 'repo-456',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:03:00Z',
            error: 'Repository permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findAllByText('Repository sync history')).toHaveLength(2);
    expect(screen.getByText('Repository id: repo-123')).toBeInTheDocument();
    const succeededSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-123' }).closest('div');
    expect(succeededSyncRow).toBeInTheDocument();
    const succeededStatusRow = within(succeededSyncRow!).getByText('Status').closest('div');
    expect(succeededStatusRow).toBeInTheDocument();
    expect(within(succeededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(succeededSyncRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(screen.getByText('Queued at: 2026-04-18T10:00:00Z')).toBeInTheDocument();
    expect(screen.getByText('Started at: 2026-04-18T10:01:00Z')).toBeInTheDocument();
    expect(screen.getByText('Finished at: 2026-04-18T10:02:00Z')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open repository detail for repo-123' })).toHaveAttribute(
      'href',
      '#/repos/repo-123'
    );
    const failedSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-456' }).closest('div');
    expect(failedSyncRow).toBeInTheDocument();
    const failedStatusRow = within(failedSyncRow!).getByText('Status').closest('div');
    expect(failedStatusRow).toBeInTheDocument();
    expect(within(failedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(failedSyncRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open repository detail for repo-456' })).toHaveAttribute(
      'href',
      '#/repos/repo-456'
    );
    expect(screen.getByText('Error: Repository permissions denied')).toBeInTheDocument();
    expect(screen.getByText('No repository sync jobs found for this connection.')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
    });
  });

  it('renders each connection\'s repository sync history newest-first when sync jobs arrive out of order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-older',
            organization_id: 'org-1',
            repository_id: 'repo-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: 'Older failure',
          },
          {
            id: 'job-other-connection',
            organization_id: 'org-1',
            repository_id: 'repo-other-connection',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:33:00Z',
            error: null,
          },
          {
            id: 'job-newer',
            organization_id: 'org-1',
            repository_id: 'repo-newer',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:02:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();
    const repositoryIds = within(githubCard!).getAllByText(/Repository id:/).map((node) => node.textContent);
    const queuedTimestamps = within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent);

    expect(repositoryIds).toEqual(['Repository id: repo-newer', 'Repository id: repo-older']);
    expect(queuedTimestamps).toEqual(['Queued at: 2026-04-18T12:00:00Z', 'Queued at: 2026-04-18T10:00:00Z']);

    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-newer · 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(githubCard!).getByText('Error: Older failure')).toBeInTheDocument();

    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(gitlabCard).toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-other-connection · 2026-04-18T11:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getByText('Repository id: repo-other-connection')).toBeInTheDocument();
  });

  it('keeps terminal-state sync-history rows scoped to their sibling authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-older',
    ]);
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-succeeded-newest')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-failed-older')).not.toBeInTheDocument();
    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('failed')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubCard!).getByText('Error: GitHub permissions denied')).toBeInTheDocument();
    expect(within(githubCard!).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-older',
    ]);
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-failed-newest')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-succeeded-older')).not.toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-succeeded-newest · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getByText('Error: GitLab import failed')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history rows newest-first by queued_at within each sibling authenticated connection card on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-oldest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-middle',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:34:00Z',
            error: 'GitLab permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-middle',
      '#/repos/repo-conn-1-succeeded-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-middle',
      '#/repos/repo-conn-2-failed-oldest',
    ]);
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T11:30:00Z',
      'Queued at: 2026-04-18T10:00:00Z',
    ]);
  });

  it('keeps sibling terminal-state sync-history rows using the shared status badge presentation truthful within each authenticated connection card when multiple rows are present', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-oldest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-middle',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:34:00Z',
            error: 'GitLab permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    const githubFailedStatusRow = within(githubFailedRow!).getByText('Status').closest('div');
    expect(githubFailedStatusRow).toBeInTheDocument();
    expect(within(githubFailedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const githubSucceededMiddleRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-middle' }).closest('div');
    expect(githubSucceededMiddleRow).toBeInTheDocument();
    const githubSucceededMiddleStatusRow = within(githubSucceededMiddleRow!).getByText('Status').closest('div');
    expect(githubSucceededMiddleStatusRow).toBeInTheDocument();
    expect(within(githubSucceededMiddleStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubSucceededMiddleRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(githubSucceededMiddleRow!).queryByText('failed')).not.toBeInTheDocument();

    const githubSucceededOldestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-oldest' }).closest('div');
    expect(githubSucceededOldestRow).toBeInTheDocument();
    const githubSucceededOldestStatusRow = within(githubSucceededOldestRow!).getByText('Status').closest('div');
    expect(githubSucceededOldestStatusRow).toBeInTheDocument();
    expect(within(githubSucceededOldestStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubSucceededOldestRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(githubSucceededOldestRow!).queryByText('failed')).not.toBeInTheDocument();

    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-newest' }).closest('div');
    expect(gitlabSucceededRow).toBeInTheDocument();
    const gitlabSucceededStatusRow = within(gitlabSucceededRow!).getByText('Status').closest('div');
    expect(gitlabSucceededStatusRow).toBeInTheDocument();
    expect(within(gitlabSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('failed')).not.toBeInTheDocument();

    const gitlabFailedMiddleRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-middle' }).closest('div');
    expect(gitlabFailedMiddleRow).toBeInTheDocument();
    const gitlabFailedMiddleStatusRow = within(gitlabFailedMiddleRow!).getByText('Status').closest('div');
    expect(gitlabFailedMiddleStatusRow).toBeInTheDocument();
    expect(within(gitlabFailedMiddleStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(gitlabFailedMiddleRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(gitlabFailedMiddleRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const gitlabFailedOldestRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-oldest' }).closest('div');
    expect(gitlabFailedOldestRow).toBeInTheDocument();
    const gitlabFailedOldestStatusRow = within(gitlabFailedOldestRow!).getByText('Status').closest('div');
    expect(gitlabFailedOldestStatusRow).toBeInTheDocument();
    expect(within(gitlabFailedOldestStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(gitlabFailedOldestRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(gitlabFailedOldestRow!).queryByText('succeeded')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards showing multiple terminal-state sync-history rows\' queued, started, and finished timestamps truthful and scoped within each card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);
    expect(within(githubCard!).getAllByText(/Started at:/).map((node) => node.textContent)).toEqual([
      'Started at: 2026-04-18T13:01:00Z',
      'Started at: 2026-04-18T11:01:00Z',
    ]);
    expect(within(githubCard!).getAllByText(/Finished at:/).map((node) => node.textContent)).toEqual([
      'Finished at: 2026-04-18T13:03:00Z',
      'Finished at: 2026-04-18T11:02:00Z',
    ]);
    expect(within(githubCard!).queryByText('Queued at: 2026-04-18T12:30:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Started at: 2026-04-18T12:31:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Finished at: 2026-04-18T10:04:00Z')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T10:00:00Z',
    ]);
    expect(within(gitlabCard!).getAllByText(/Started at:/).map((node) => node.textContent)).toEqual([
      'Started at: 2026-04-18T12:31:00Z',
      'Started at: 2026-04-18T10:01:00Z',
    ]);
    expect(within(gitlabCard!).getAllByText(/Finished at:/).map((node) => node.textContent)).toEqual([
      'Finished at: 2026-04-18T12:33:00Z',
      'Finished at: 2026-04-18T10:04:00Z',
    ]);
    expect(within(gitlabCard!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Started at: 2026-04-18T11:01:00Z')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Finished at: 2026-04-18T13:03:00Z')).not.toBeInTheDocument();
  });

  it('renders terminal-state sync-history rows with the shared status badge presentation across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('failed')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-failed · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).queryByText('Latest sync: failed')).not.toBeInTheDocument();

    const githubTerminalSyncRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed' }).closest('div');
    expect(githubTerminalSyncRow).toBeInTheDocument();
    const githubStatusRow = within(githubTerminalSyncRow!).getByText('Status').closest('div');
    expect(githubStatusRow).toBeInTheDocument();
    expect(within(githubStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Status: failed')).not.toBeInTheDocument();

    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-succeeded · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).queryByText('Latest sync: succeeded')).not.toBeInTheDocument();

    const gitlabTerminalSyncRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded' }).closest('div');
    expect(gitlabTerminalSyncRow).toBeInTheDocument();
    const gitlabStatusRow = within(gitlabTerminalSyncRow!).getByText('Status').closest('div');
    expect(gitlabStatusRow).toBeInTheDocument();
    expect(within(gitlabStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubTerminalSyncRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed' }).closest('div');
    expect(githubTerminalSyncRow).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Finished at: 2026-04-18T13:03:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Queued at: 2026-04-18T12:30:00Z')).not.toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Started at: 2026-04-18T12:31:00Z')).not.toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Finished at: 2026-04-18T12:33:00Z')).not.toBeInTheDocument();

    const gitlabTerminalSyncRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded' }).closest('div');
    expect(gitlabTerminalSyncRow).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Queued at: 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Started at: 2026-04-18T12:31:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Finished at: 2026-04-18T12:33:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Finished at: 2026-04-18T13:03:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history rows on the same authenticated connection card using the shared status badge presentation truthfully and newest-first by queued_at', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror permissions denied',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-succeeded-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);

    const newestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-newest' }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    const newestSucceededStatusRow = within(newestSucceededRow!).getByText('Status').closest('div');
    expect(newestSucceededStatusRow).toBeInTheDocument();
    expect(within(newestSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('failed')).not.toBeInTheDocument();

    const middleFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-middle' }).closest('div');
    expect(middleFailedRow).toBeInTheDocument();
    const middleFailedStatusRow = within(middleFailedRow!).getByText('Status').closest('div');
    expect(middleFailedStatusRow).toBeInTheDocument();
    expect(within(middleFailedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const oldestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-oldest' }).closest('div');
    expect(oldestSucceededRow).toBeInTheDocument();
    const oldestSucceededStatusRow = within(oldestSucceededRow!).getByText('Status').closest('div');
    expect(oldestSucceededStatusRow).toBeInTheDocument();
    expect(within(oldestSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('failed')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history timestamp details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror permissions denied',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-succeeded-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-succeeded-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();

    const newestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-newest' }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const middleFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-middle' }).closest('div');
    expect(middleFailedRow).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Started at: 2026-04-18T11:03:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Finished at: 2026-04-18T11:07:00Z')).not.toBeInTheDocument();

    const oldestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-oldest' }).closest('div');
    expect(oldestSucceededRow).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Queued at: 2026-04-18T11:00:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Started at: 2026-04-18T11:03:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Finished at: 2026-04-18T11:07:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history error details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Newest mirror fetch failed',
          },
          {
            id: 'job-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-oldest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: 'Oldest mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-failed-newest',
      '#/repos/repo-succeeded-middle',
      '#/repos/repo-failed-oldest',
    ]);

    const newestFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-newest' }).closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Newest mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Oldest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const middleSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-middle' }).closest('div');
    expect(middleSucceededRow).toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Error: Newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Error: Oldest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();

    const oldestFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-oldest' }).closest('div');
    expect(oldestFailedRow).toBeInTheDocument();
    expect(within(oldestFailedRow!).getByText('Error: Oldest mirror fetch failed')).toBeInTheDocument();
    expect(within(oldestFailedRow!).queryByText('Error: Newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(oldestFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history error details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub newest mirror fetch failed',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:35:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: 'GitLab oldest import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-older',
    ]);
    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    expect(within(githubFailedRow!).getByText('Error: GitHub newest mirror fetch failed')).toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    const githubSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-older' }).closest('div');
    expect(githubSucceededRow).toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-older',
    ]);
    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-newest' }).closest('div');
    expect(gitlabSucceededRow).toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    const gitlabFailedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-older' }).closest('div');
    expect(gitlabFailedRow).toBeInTheDocument();
    expect(within(gitlabFailedRow!).getByText('Error: GitLab oldest import failed')).toBeInTheDocument();
    expect(within(gitlabFailedRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps identical terminal-state sync-history error strings isolated across sibling authenticated connection cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-newest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:35:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-older',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    const githubSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-older' }).closest('div');
    const gitlabFailedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-newest' }).closest('div');
    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-older' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    expect(githubSucceededRow).toBeInTheDocument();
    expect(gitlabFailedRow).toBeInTheDocument();
    expect(gitlabSucceededRow).toBeInTheDocument();

    expect(within(githubFailedRow!).getByLabelText('Error details for repo-conn-1-failed-newest')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(gitlabFailedRow!).getByLabelText('Error details for repo-conn-2-failed-newest')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(githubSucceededRow!).queryByLabelText('Error details for repo-conn-1-succeeded-older')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByLabelText('Error details for repo-conn-2-succeeded-older')).not.toBeInTheDocument();

    expect(within(githubFailedRow!).queryByLabelText('Error details for repo-conn-2-failed-newest')).not.toBeInTheDocument();
    expect(within(gitlabFailedRow!).queryByLabelText('Error details for repo-conn-1-failed-newest')).not.toBeInTheDocument();
    expect(within(githubCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(1);
    expect(within(gitlabCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(1);
  });

  it('keeps identical terminal-state sync-history error strings isolated within multiple failed rows on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-failed-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);

    const newestFailedRow = within(githubCard!).getByLabelText('Repository sync history row for repo-failed-newest');
    const middleFailedRow = within(githubCard!).getByLabelText('Repository sync history row for repo-failed-middle');
    const succeededRow = within(githubCard!).getByLabelText('Repository sync history row for repo-succeeded-oldest');

    expect(within(newestFailedRow).getByLabelText('Error details for repo-failed-newest')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(middleFailedRow).getByLabelText('Error details for repo-failed-middle')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(succeededRow).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();

    expect(within(newestFailedRow).queryByLabelText('Error details for repo-failed-middle')).not.toBeInTheDocument();
    expect(within(middleFailedRow).queryByLabelText('Error details for repo-failed-newest')).not.toBeInTheDocument();
    expect(within(newestFailedRow).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(2);
  });

  it('keeps the latest-sync summary truthful when multiple failed rows on the same authenticated connection card share the same terminal error string', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('failed');
    expect(latestSyncSummary).toHaveTextContent('repo-failed-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps each sibling authenticated connection card latest-sync summary truthful to its own newest failed row when terminal error strings match', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-newest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:30:00Z',
            started_at: '2026-04-18T13:31:00Z',
            finished_at: '2026-04-18T13:35:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-conn-1-failed-older · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-conn-2-failed-newest · 2026-04-18T13:30:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-conn-2-failed-newest · 2026-04-18T13:30:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-conn-2-failed-older · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories end in failed-vs-succeeded newest rows', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:30:00Z',
            started_at: '2026-04-18T13:31:00Z',
            finished_at: '2026-04-18T13:35:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T13:30:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-history · 2026-04-18T13:30:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-older-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T11:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards and the API returns those tied newest rows in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-succeeded-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-succeeded-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards, reuse the same repository id, and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-same-timestamp · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-same-timestamp · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-same-timestamp-stable-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-same-timestamp-stable-order · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards terminal-state sync-history timestamp details truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp-stable-order-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-github-older-shared-repo-same-timestamp-stable-order-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-gitlab-older-shared-repo-same-timestamp-stable-order-details',
    ]);

    const githubNewestTerminalRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-details' }).closest('div');
    expect(githubNewestTerminalRow).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabNewestTerminalRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-details' }).closest('div');
    expect(gitlabNewestTerminalRow).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-succeeded-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-shared-older-stable-order-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-stable-order-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-stable-order-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-shared-older-stable-order-error-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-stable-order-error-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-stable-order-error-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Older mirror fetch failed')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has an older failed row but a newer succeeded terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-succeeded-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has an older succeeded row but a newer failed terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('failed');
    expect(latestSyncSummary).toHaveTextContent('repo-failed-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('succeeded');
    expect(latestSyncSummary).not.toHaveTextContent('repo-succeeded-older · 2026-04-18T12:00:00Z');
  });

  it('renders queued and running sync-history rows with the shared status badge presentation on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running',
            organization_id: 'org-1',
            repository_id: 'repo-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued',
            organization_id: 'org-1',
            repository_id: 'repo-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-running · 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(latestSyncSummary!).queryByText('Latest sync: running')).not.toBeInTheDocument();

    const runningSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-running' }).closest('div');
    expect(runningSyncRow).toBeInTheDocument();
    const runningStatusRow = within(runningSyncRow!).getByText('Status').closest('div');
    expect(runningStatusRow).toBeInTheDocument();
    expect(within(runningStatusRow!).getByText('running')).toBeInTheDocument();
    expect(within(runningSyncRow!).queryByText('Status: running')).not.toBeInTheDocument();
    expect(within(runningSyncRow!).getByText('Started at: 2026-04-18T12:01:00Z')).toBeInTheDocument();
    expect(within(runningSyncRow!).getByText('Finished at: Not finished')).toBeInTheDocument();

    const queuedSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-queued' }).closest('div');
    expect(queuedSyncRow).toBeInTheDocument();
    const queuedStatusRow = within(queuedSyncRow!).getByText('Status').closest('div');
    expect(queuedStatusRow).toBeInTheDocument();
    expect(within(queuedStatusRow!).getByText('queued')).toBeInTheDocument();
    expect(within(queuedSyncRow!).queryByText('Status: queued')).not.toBeInTheDocument();
    expect(within(queuedSyncRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedSyncRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
  });

  it('keeps multiple queued and running sync-history rows newest-first by queued_at on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-queued-oldest',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-middle',
            organization_id: 'org-1',
            repository_id: 'repo-running-middle',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const syncHistoryLinks = within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ });
    expect(syncHistoryLinks.map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest',
      '#/repos/repo-running-middle',
      '#/repos/repo-queued-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-running-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at timestamp and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-older-identical-activity-shared-repo-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-older-identical-activity-shared-repo-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows tying on both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-newest-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-shared-repo-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity-shared-repo-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity · 2026-04-18T12:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share the same newest queued_at timestamp across cards and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-same-timestamp',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older · 2026-04-18T11:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share both newest queued_at and activity timestamps across cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-identical-activity',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-identical-activity',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-identical-activity · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-identical-activity · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-identical-activity · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-identical-activity · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share both newest queued_at and activity timestamps across cards and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-queued-older-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows tie on both newest queued_at and activity timestamps, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-queued-older-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-shared-repo-identical-activity-reverse-order · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-shared-repo-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows tie on both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running-newest-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-identical-activity-stable-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-shared-repo-identical-activity-stable-order · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-shared-repo-identical-activity-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps multiple queued and running sync-history timestamp details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-middle',
            organization_id: 'org-1',
            repository_id: 'repo-queued-middle',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-running-oldest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest',
      '#/repos/repo-queued-middle',
      '#/repos/repo-running-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const newestRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-newest' }).closest('div');
    expect(newestRunningRow).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(newestRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestRunningRow!).queryByText('Started at: 2026-04-18T11:01:00Z')).not.toBeInTheDocument();

    const middleQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-queued-middle' }).closest('div');
    expect(middleQueuedRow).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(middleQueuedRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(middleQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const oldestRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-oldest' }).closest('div');
    expect(oldestRunningRow).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Queued at: 2026-04-18T11:00:00Z')).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Started at: 2026-04-18T11:01:00Z')).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(oldestRunningRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestRunningRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-running-older-identical-activity-details',
    ]);

    const runningNewestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const queuedNewestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-newest-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-stable-order-shared-repo-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-running-older-identical-activity-stable-order-shared-repo-details',
    ]);

    const sharedRepoRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-same-card-shared-newest-identical-activity-stable-order-details',
    });
    expect(sharedRepoRows).toHaveLength(2);

    const runningNewestRow = sharedRepoRows[0].closest('div');
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const queuedNewestRow = sharedRepoRows[1].closest('div');
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued and running sync-history rows scoped to their owning connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T10:30:00Z',
            started_at: '2026-04-18T10:31:00Z',
            finished_at: '2026-04-18T10:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-running',
      '#/repos/repo-conn-1-queued',
    ]);
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-queued')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-running')).not.toBeInTheDocument();
    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-running · 2026-04-18T13:00:00Z')).toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-queued',
      '#/repos/repo-conn-2-running',
      '#/repos/repo-conn-2-succeeded',
    ]);
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-running')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-queued')).not.toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-queued · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T11:30:00Z',
      'Queued at: 2026-04-18T10:30:00Z',
    ]);
  });

  it('keeps in-progress sync-history timestamp details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T10:30:00Z',
            started_at: '2026-04-18T10:31:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-running' }).closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T11:30:00Z')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: 2026-04-18T10:31:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-queued' }).closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T10:30:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-queued' }).closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T11:30:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const gitlabRunningRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-running' }).closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T10:30:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T10:31:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-github-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-older',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-gitlab-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-running-older',
    ]);

    const githubRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-github-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-github-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabRunningRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-gitlab-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-gitlab-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-older-shared-repo-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-running-older-shared-repo-details',
    ]);

    const githubSharedRepoRows = within(githubCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-reverse-order-details' });
    expect(githubSharedRepoRows).toHaveLength(2);
    const githubRunningRow = githubSharedRepoRows[0].closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = githubSharedRepoRows[1].closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabSharedRepoRows = within(gitlabCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-reverse-order-details' });
    expect(gitlabSharedRepoRows).toHaveLength(2);
    const gitlabRunningRow = gitlabSharedRepoRows[0].closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = gitlabSharedRepoRows[1].closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-stable-order-shared-repo-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-stable-order-shared-repo-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-github-queued-older-stable-order-shared-repo-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-gitlab-running-older-stable-order-shared-repo-details',
    ]);

    const githubSharedRepoRows = within(githubCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-stable-order-details' });
    expect(githubSharedRepoRows).toHaveLength(2);
    const githubRunningRow = githubSharedRepoRows[0].closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = githubSharedRepoRows[1].closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabSharedRepoRows = within(gitlabCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-stable-order-details' });
    expect(gitlabSharedRepoRows).toHaveLength(2);
    const gitlabRunningRow = gitlabSharedRepoRows[0].closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = gitlabSharedRepoRows[1].closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps the empty sync-history state on one authenticated connection card while another shows queued and running rows on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getByText('No repository sync jobs found for this connection.')).toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-running · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-running',
      '#/repos/repo-conn-2-queued',
    ]);
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
    ]);
  });

  it('keeps the sync-history loading state visible and truthful on each authenticated connection card before sync jobs resolve', async () => {
    window.location.hash = '#/settings/connections';

    const deferredSyncJobs = deferredResponse();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return await deferredSyncJobs.promise;
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getByText('Loading repository sync history…')).toBeInTheDocument();
    expect(within(githubCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    expect(within(gitlabCard!).getByText('Loading repository sync history…')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    deferredSyncJobs.resolve(jsonResponse([]));
    expect(await screen.findAllByText('No repository sync jobs found for this connection.')).toHaveLength(2);
  });

  it('keeps the sync-history failure state visible and truthful on each authenticated connection card without collapsing sibling cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse({}, false, 503);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();

    const expectedError = 'Failed to load repository sync history: Request failed: 503';
    expect(await screen.findAllByText(expectedError)).toHaveLength(2);

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const genericCard = screen.getByText('Generic Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(genericCard).toBeInTheDocument();

    for (const card of [githubCard, genericCard]) {
      expect(card).not.toBeNull();
      expect(within(card!).getByText(expectedError)).toBeInTheDocument();
      expect(within(card!).queryByText('Loading repository sync history…')).not.toBeInTheDocument();
      expect(within(card!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
      expect(within(card!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
      expect(within(card!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();
    }

    expect(within(githubCard!).getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(within(genericCard!).getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(within(genericCard!).getByText('Repository discovery is not available yet for generic Git connections.')).toBeInTheDocument();
  });

  it('shows a generic git quick-open affordance on the settings route while keeping discovery status truthful', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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
            name: 'Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
          {
            id: 'conn-3',
            name: 'Unsafe Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'javascript:alert(1)',
            },
          },
          {
            id: 'conn-4',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const genericCard = screen.getByText('Generic Mirror').closest('article');
    const unsafeGenericCard = screen.getByText('Unsafe Generic Mirror').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    expect(githubCard).toBeInTheDocument();
    expect(genericCard).toBeInTheDocument();
    expect(unsafeGenericCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();

    expect(within(genericCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(genericCard!).getByText('Repository discovery is not available yet for generic Git connections.')
    ).toBeInTheDocument();
    const genericQuickOpenLink = within(genericCard!).getByRole('link', { name: 'Open host for manual discovery' });
    expect(genericQuickOpenLink).toHaveAttribute('href', 'https://git.internal.example.com/');
    expect(genericQuickOpenLink).toHaveAttribute('target', '_blank');
    expect(genericQuickOpenLink).toHaveAttribute('rel', 'noreferrer');
    expect(within(unsafeGenericCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(unsafeGenericCard!).getByText('Repository discovery is not available yet for generic Git connections.')
    ).toBeInTheDocument();
    expect(within(unsafeGenericCard!).queryByRole('link', { name: 'Open host for manual discovery' })).not.toBeInTheDocument();
    expect(within(localCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(localCard!).getByText('Import one repository path at a time from this local root.')
    ).toBeInTheDocument();
    expect(
      within(localCard!).getByText('Recursive local enumeration is not available yet.')
    ).toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open host for manual discovery' })).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Discovery status')).not.toBeInTheDocument();
  });

  it('shows a local-only repository import form on the settings route and renders imported repository details on success', async () => {
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-2',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    expect(githubCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();
    expect(within(githubCard!).queryByLabelText('Repository path')).not.toBeInTheDocument();
    expect(within(localCard!).getByLabelText('Repository path')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();
    expect(within(localCard!).getByText('Default branch: main')).toBeInTheDocument();
    expect(within(localCard!).getByText('Sync state')).toBeInTheDocument();
    expect(within(localCard!).getByText('ready')).toBeInTheDocument();
    expect(within(localCard!).queryByText('Sync state: ready')).not.toBeInTheDocument();
    expect(within(localCard!).getByText('Kind: local')).toBeInTheDocument();
    expect(within(localCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).getByRole('link', { name: 'Open repository detail' })).toHaveAttribute(
      'href',
      '#/repos/repo-77'
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-2',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
    });
  });

  it('clears stale imported repository details after the local import path changes', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-beta' },
    });

    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
  });

  it('restores the local import path to the configured connection root', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();
    expect(within(localCard!).queryByRole('button', { name: 'Reset to local root' })).not.toBeInTheDocument();

    const pathInput = within(localCard!).getByLabelText('Repository path') as HTMLInputElement;
    fireEvent.change(pathInput, {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();

    const resetButton = within(localCard!).getByRole('button', { name: 'Reset to local root' });
    fireEvent.click(resetButton);

    expect(pathInput.value).toBe('/srv/git/mirror');
    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('button', { name: 'Reset to local root' })).not.toBeInTheDocument();
  });

  it('disables local connection edit and delete controls while a repository import is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveImport: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return await new Promise<Response>((resolve) => {
          resolveImport = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Importing…' })).toBeDisabled();
    });
    expect(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' })).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' })).toBeDisabled();
    expect(within(githubCard!).getByRole('button', { name: 'Edit GitHub Cloud' })).not.toBeDisabled();
    expect(within(githubCard!).getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();

    resolveImport?.(
      jsonResponse({
        repository: {
          id: 'repo-77',
          name: 'project-alpha',
          default_branch: 'main',
          connection_id: 'conn-2',
          sync_state: 'ready',
        },
        connection: {
          id: 'conn-2',
          name: 'Local Mirror',
          kind: 'local',
        },
      })
    );

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' })).not.toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' })).not.toBeDisabled();
  });

  it('disables an open local connection edit form while a repository import is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveImport: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return await new Promise<Response>((resolve) => {
          resolveImport = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Importing…' })).toBeDisabled();
    });
    expect(screen.getByLabelText('Edit connection name')).toBeDisabled();
    expect(screen.getByLabelText('Edit repo path')).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save changes' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();

    resolveImport?.(
      jsonResponse({
        repository: {
          id: 'repo-77',
          name: 'project-alpha',
          default_branch: 'main',
          connection_id: 'conn-2',
          sync_state: 'ready',
        },
        connection: {
          id: 'conn-2',
          name: 'Local Mirror',
          kind: 'local',
        },
      })
    );

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(screen.getByLabelText('Edit connection name')).not.toBeDisabled();
    expect(screen.getByLabelText('Edit repo path')).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save changes' })).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).not.toBeDisabled();
  });

  it('disables a local connection import form while that connection update is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveUpdate: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'PUT') {
        return await new Promise<Response>((resolve) => {
          resolveUpdate = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
    });
    expect(within(localCard!).getByLabelText('Repository path')).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).toBeDisabled();

    resolveUpdate?.(
      jsonResponse({
        id: 'conn-2',
        name: 'Local Mirror',
        kind: 'local',
        config: {
          provider: 'local',
          repo_path: '/srv/git/updated-root',
        },
      })
    );

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).not.toBeDisabled();
  });

  it('disables a local connection import form while that connection deletion is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    });
    expect(within(localCard!).getByLabelText('Repository path')).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));
    await waitFor(() => {
      expect(screen.queryByText('Local Mirror')).not.toBeInTheDocument();
    });
  });

  it('shows a scoped repository import failure on the affected local connection card only', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({}, false, 422);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByLabelText('Repository path')).not.toBeInTheDocument();
  });

  it('clears stale local import failure details after the repository path changes', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
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

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({}, false, 422);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });

    expect(within(localCard!).queryByText('Failed to import repository: Request failed: 422')).not.toBeInTheDocument();
  });

  it('resets a local connection import form to the updated repo root after editing that connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-1',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('clears stale imported repository success details after editing a local connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-1',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();
    expect(within(localCard!).getByRole('link', { name: 'Open repository detail' })).toHaveAttribute(
      'href',
      '#/repos/repo-77'
    );

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('clears stale local import failure details after editing a local connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/missing-repo',
          })
        );

        return jsonResponse({}, false, 422);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/missing-repo',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Failed to import repository: Request failed: 422')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('edits an authenticated github connection from the settings route using the update api', async () => {
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

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' GitHub Enterprise ',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.enterprise.example.com',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'GitHub Enterprise',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.enterprise.example.com',
          },
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit GitHub Cloud' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' GitHub Enterprise ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://github.enterprise.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByText('GitHub Enterprise')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.enterprise.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: github')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-1')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' GitHub Enterprise ',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.enterprise.example.com',
          },
        }),
      });
    });
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('edits an authenticated generic git connection from the settings route using the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
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

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Generic Git Host ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Generic Git Host',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit Generic Git Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' Generic Git Host ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://git.internal.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByText('Generic Git Host')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: generic_git')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-1')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Generic Git Host ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('shows scoped generic git connection edit failures on the settings route while keeping the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Scoped generic failure ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const connectionCard = screen.getByText('Generic Git Mirror').closest('article');
    expect(connectionCard).not.toBeNull();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit Generic Git Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' Scoped generic failure ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://git.internal.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await within(connectionCard as HTMLElement).findByText('Failed to update connection: Request failed: 403')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.queryByText('Scoped generic failure')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Edit connection name')).toHaveValue(' Scoped generic failure ');
    expect(screen.getByLabelText('Edit base URL')).toHaveValue('https://git.internal.example.com');
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Scoped generic failure ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
  });

  it('shows scoped generic git connection delete failures on the settings route while keeping the host-style inventory visible', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'DELETE') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const genericConnectionCard = screen.getByText('Generic Git Mirror').closest('article');
    expect(genericConnectionCard).not.toBeNull();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Generic Git Mirror' }));

    expect(
      await within(genericConnectionCard as HTMLElement).findByText('Failed to delete connection: Request failed: 403')
    ).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Deleting…' })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'DELETE',
      });
    });
  });

  it('removes only the deleted generic git connection from the settings route while keeping the remaining host-style inventory visible', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Generic Git Mirror' }));

    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));

    await waitFor(() => {
      expect(screen.queryByText('Generic Git Mirror')).not.toBeInTheDocument();
    });
    expect(screen.queryByText('Base URL: https://git.example.com')).not.toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'DELETE',
      });
    });
  });

  it('disables all authenticated connection delete controls while a deletion is in flight and removes the deleted connection', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
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

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Local Mirror' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-2', {
        method: 'DELETE',
      });
    });
    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));

    await waitFor(() => {
      expect(screen.queryByText('Local Mirror')).not.toBeInTheDocument();
    });
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();
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

  it('shows scoped generic git connection creation failures on the settings route while keeping the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Scoped generic failure',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: 'Scoped generic failure' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'generic_git' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://git.internal.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Failed to create connection: Request failed: 403')).toBeInTheDocument();
    expect(screen.queryByText('Scoped generic failure')).not.toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Scoped generic failure',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
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
