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
