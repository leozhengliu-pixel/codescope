import { useEffect, useMemo, useState, type CSSProperties, type ReactNode } from 'react';

type SyncState = 'pending' | 'ready' | 'error';

type RepoSummary = {
  id: string;
  name: string;
  default_branch: string;
  sync_state: SyncState;
};

type RepoDetail = {
  repository: {
    id: string;
    name: string;
    default_branch: string;
    connection_id: string;
    sync_state: SyncState;
  };
  connection: {
    id: string;
    name: string;
    kind: string;
  };
};

type BrowseEntry = {
  name: string;
  path: string;
  kind: 'dir' | 'file';
};

type TreeResponse = {
  repo_id: string;
  path: string;
  entries: BrowseEntry[];
};

type BlobResponse = {
  repo_id: string;
  path: string;
  content: string;
  size_bytes: number;
};

type SearchResult = {
  repo_id: string;
  path: string;
  line_number: number;
  line: string;
};

type SearchResponse = {
  query: string;
  repo_id: string | null;
  results: SearchResult[];
};

type CommitSummary = {
  id: string;
  short_id: string;
  summary: string;
  author_name: string;
  authored_at: string;
};

type CommitsResponse = {
  repo_id: string;
  commits: CommitSummary[];
};

type CommitDetail = CommitSummary & {
  body?: string;
  parents: string[];
};

type CommitResponse = {
  repo_id: string;
  commit: CommitDetail;
};

function useHashLocation() {
  const [hash, setHash] = useState(() => window.location.hash || '#/');

  useEffect(() => {
    const onChange = () => setHash(window.location.hash || '#/');
    window.addEventListener('hashchange', onChange);
    return () => window.removeEventListener('hashchange', onChange);
  }, []);

  return hash;
}

async function fetchJson<T>(path: string): Promise<T> {
  const response = await fetch(path);
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status}`);
  }
  return (await response.json()) as T;
}

function StatusBadge({ state }: { state: SyncState }) {
  const colors: Record<SyncState, string> = {
    pending: '#9a6700',
    ready: '#1a7f37',
    error: '#cf222e',
  };

  return (
    <span
      style={{
        display: 'inline-block',
        padding: '2px 10px',
        borderRadius: 999,
        background: `${colors[state]}18`,
        color: colors[state],
        fontSize: 12,
        fontWeight: 600,
        textTransform: 'uppercase',
      }}
    >
      {state}
    </span>
  );
}

function RepoListPage() {
  const [repos, setRepos] = useState<RepoSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [selectedRepoId, setSelectedRepoId] = useState('');
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [submittedQuery, setSubmittedQuery] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<RepoSummary[]>('/api/v1/repos')
      .then((data) => {
        if (!cancelled) {
          setRepos(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const repoNamesById = useMemo(() => new Map(repos.map((repo) => [repo.id, repo.name])), [repos]);

  const runSearch = async () => {
    const trimmedQuery = query.trim();
    if (!trimmedQuery) {
      setSubmittedQuery(null);
      setSearchResults([]);
      setSearchError(null);
      return;
    }

    setSearchLoading(true);

    try {
      const params = new URLSearchParams({ q: trimmedQuery, repo_id: selectedRepoId });
      const data = await fetchJson<SearchResponse>(`/api/v1/search?${params.toString()}`);
      setSearchResults(data.results);
      setSubmittedQuery(data.query);
      setSearchError(null);
    } catch (err) {
      setSearchResults([]);
      setSubmittedQuery(trimmedQuery);
      setSearchError((err as Error).message);
    } finally {
      setSearchLoading(false);
    }
  };

  if (loading) return <Panel title="Repositories">Loading repositories…</Panel>;
  if (error) return <Panel title="Repositories">Failed to load: {error}</Panel>;

  return (
    <div style={{ display: 'grid', gap: 20 }}>
      <Panel title="Search" subtitle="Search indexed code across repositories using the clean-room API.">
        <form
          style={{ display: 'grid', gap: 12 }}
          onSubmit={(event) => {
            event.preventDefault();
            void runSearch();
          }}
        >
          <div style={searchFormGridStyle}>
            <label style={fieldLabelStyle}>
              <span>Search query</span>
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search for symbols, strings, or snippets"
                style={inputStyle}
              />
            </label>

            <label style={fieldLabelStyle}>
              <span>Repository filter</span>
              <select
                value={selectedRepoId}
                onChange={(event) => setSelectedRepoId(event.target.value)}
                style={inputStyle}
              >
                <option value="">All repositories</option>
                {repos.map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repo.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div>
            <button type="submit" style={primaryButtonStyle} disabled={searchLoading || query.trim().length === 0}>
              {searchLoading ? 'Searching…' : 'Search'}
            </button>
          </div>
        </form>

        <div style={{ marginTop: 20 }}>
          <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>Results</div>
          {searchLoading ? <div>Searching code…</div> : null}
          {!searchLoading && searchError ? <div>Search failed: {searchError}</div> : null}
          {!searchLoading && !searchError && submittedQuery && searchResults.length === 0 ? (
            <div>No matches found for “{submittedQuery}”.</div>
          ) : null}
          {!searchLoading && !searchError && searchResults.length > 0 ? (
            <div style={{ display: 'grid', gap: 12 }}>
              {searchResults.map((result, index) => (
                <div key={`${result.repo_id}:${result.path}:${result.line_number}:${index}`} style={searchResultCardStyle}>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginBottom: 8 }}>
                    <span style={searchMetaBadgeStyle}>{repoNamesById.get(result.repo_id) ?? result.repo_id}</span>
                    <span style={searchMetaBadgeStyle}>{result.path}</span>
                    <span style={searchMetaBadgeStyle}>Line {result.line_number}</span>
                  </div>
                  <pre style={searchLineStyle}>{result.line}</pre>
                </div>
              ))}
            </div>
          ) : null}
          {!searchLoading && !searchError && !submittedQuery ? (
            <div style={{ color: '#57606a' }}>Enter a query to search indexed code.</div>
          ) : null}
        </div>
      </Panel>

      <Panel title="Repositories" subtitle="Seeded repository inventory from the clean-room API.">
        <div style={{ display: 'grid', gap: 12 }}>
          {repos.map((repo) => (
            <a
              key={repo.id}
              href={`#/repos/${repo.id}`}
              style={{
                padding: 16,
                border: '1px solid #d0d7de',
                borderRadius: 12,
                color: 'inherit',
                textDecoration: 'none',
                background: '#fff',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
                <div>
                  <div style={{ fontSize: 18, fontWeight: 700 }}>{repo.name}</div>
                  <div style={{ color: '#57606a', marginTop: 4 }}>Default branch: {repo.default_branch}</div>
                </div>
                <StatusBadge state={repo.sync_state} />
              </div>
            </a>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function RepoDetailPage({ repoId }: { repoId: string }) {
  const [repo, setRepo] = useState<RepoDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    fetchJson<RepoDetail>(`/api/v1/repos/${repoId}`)
      .then((data) => {
        if (!cancelled) {
          setRepo(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setError(err.message);
          setRepo(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [repoId]);

  if (loading) return <Panel title="Repository detail">Loading repository…</Panel>;
  if (error) return <Panel title="Repository detail">Failed to load: {error}</Panel>;
  if (!repo) return <Panel title="Repository detail">Repository not found.</Panel>;

  return (
    <Panel title={repo.repository.name} subtitle={`Repository id: ${repo.repository.id}`}>
      <div style={detailGridStyle}>
        <Detail label="Default branch" value={repo.repository.default_branch} />
        <Detail label="Sync state" value={<StatusBadge state={repo.repository.sync_state} />} />
        <Detail label="Connection" value={repo.connection.name} />
        <Detail label="Connection kind" value={repo.connection.kind} />
      </div>
      <div style={{ marginTop: 20 }}>
        <CommitsPanel repoId={repoId} />
      </div>
      <div style={{ marginTop: 20 }}>
        <BrowsePanel repoId={repoId} />
      </div>
      <div style={{ marginTop: 16 }}>
        <a href="#/" style={{ color: '#0969da', textDecoration: 'none', fontWeight: 600 }}>
          ← Back to repositories
        </a>
      </div>
    </Panel>
  );
}

function CommitsPanel({ repoId }: { repoId: string }) {
  const [commits, setCommits] = useState<CommitSummary[]>([]);
  const [commitsLoading, setCommitsLoading] = useState(true);
  const [commitsError, setCommitsError] = useState<string | null>(null);
  const [selectedCommitId, setSelectedCommitId] = useState<string | null>(null);
  const [commitDetail, setCommitDetail] = useState<CommitDetail | null>(null);
  const [commitLoading, setCommitLoading] = useState(false);
  const [commitError, setCommitError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setCommitsLoading(true);
    setSelectedCommitId(null);
    setCommitDetail(null);
    setCommitError(null);
    setCommitLoading(false);

    fetchJson<CommitsResponse>(`/api/v1/repos/${repoId}/commits?limit=20`)
      .then((data) => {
        if (!cancelled) {
          setCommits(data.commits);
          setCommitsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setCommits([]);
          setCommitsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setCommitsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [repoId]);

  useEffect(() => {
    let cancelled = false;

    if (!selectedCommitId) {
      setCommitDetail(null);
      setCommitError(null);
      setCommitLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setCommitLoading(true);
    setCommitError(null);

    fetchJson<CommitResponse>(`/api/v1/repos/${repoId}/commits/${encodeURIComponent(selectedCommitId)}`)
      .then((data) => {
        if (!cancelled) {
          setCommitDetail(data.commit);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setCommitDetail(null);
          setCommitError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setCommitLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [repoId, selectedCommitId]);

  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'minmax(260px, 1fr) minmax(0, 2fr)', gap: 12 }}>
      <section style={browseSectionStyle}>
        <div style={browseSectionHeaderStyle}>
          <div>
            <div style={browseSectionTitleStyle}>Recent commits</div>
            <div style={browseSectionMetaStyle}>Latest 20 commits from the repository API.</div>
          </div>
        </div>

        {commitsLoading ? <div>Loading commits…</div> : null}
        {!commitsLoading && commitsError ? <div>Unable to load commits: {commitsError}</div> : null}
        {!commitsLoading && !commitsError && commits.length === 0 ? <div>No commits found.</div> : null}
        {!commitsLoading && !commitsError && commits.length > 0 ? (
          <div style={{ display: 'grid', gap: 8 }}>
            {commits.map((commit) => (
              <button
                key={commit.id}
                type="button"
                onClick={() => setSelectedCommitId(commit.id)}
                style={{
                  ...entryButtonStyle,
                  alignItems: 'flex-start',
                  flexDirection: 'column',
                  background: selectedCommitId === commit.id ? '#ddf4ff' : '#fff',
                }}
              >
                <div style={{ fontWeight: 700 }}>{commit.summary}</div>
                <div style={browseSectionMetaStyle}>{commit.author_name}</div>
                <div style={{ ...browseSectionMetaStyle, marginTop: 0 }}>{shortCommitId(commit.id)}</div>
              </button>
            ))}
          </div>
        ) : null}
      </section>

      <section style={browseSectionStyle}>
        <div style={browseSectionHeaderStyle}>
          <div>
            <div style={browseSectionTitleStyle}>Commit details</div>
            <div style={browseSectionMetaStyle}>{commitDetail?.id ?? selectedCommitId ?? 'No commit selected yet.'}</div>
          </div>
        </div>

        {commitLoading ? <div>Loading commit…</div> : null}
        {!commitLoading && commitError ? <div>Unable to load commit: {commitError}</div> : null}
        {!commitLoading && !commitError && commitDetail ? (
          <div style={{ display: 'grid', gap: 12 }}>
            <div style={detailCardStyle}>
              <div style={{ fontSize: 18, fontWeight: 700 }}>{commitDetail.summary}</div>
              {commitDetail.body ? <div style={{ ...browseSectionMetaStyle, whiteSpace: 'pre-wrap' }}>{commitDetail.body}</div> : null}
            </div>
            <div style={detailGridStyle}>
              <Detail label="Author" value={commitDetail.author_name} />
              <Detail label="Time" value={commitDetail.authored_at} />
              <Detail
                label="Parents"
                value={
                  commitDetail.parents.length > 0 ? (
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {commitDetail.parents.map((parent) => (
                        <span key={parent} style={searchMetaBadgeStyle}>
                          {parent}
                        </span>
                      ))}
                    </div>
                  ) : (
                    'None'
                  )
                }
              />
            </div>
          </div>
        ) : null}
        {!commitLoading && !commitError && !commitDetail ? (
          <div style={{ color: '#57606a' }}>Select a commit to inspect its details.</div>
        ) : null}
      </section>
    </div>
  );
}

function BrowsePanel({ repoId }: { repoId: string }) {
  const [treePath, setTreePath] = useState('');
  const [tree, setTree] = useState<TreeResponse | null>(null);
  const [treeLoading, setTreeLoading] = useState(true);
  const [treeError, setTreeError] = useState<string | null>(null);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [blob, setBlob] = useState<BlobResponse | null>(null);
  const [blobLoading, setBlobLoading] = useState(false);
  const [blobError, setBlobError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setTreeLoading(true);

    fetchJson<TreeResponse>(`/api/v1/repos/${repoId}/tree?path=${encodeURIComponent(treePath)}`)
      .then((data) => {
        if (!cancelled) {
          setTree(data);
          setTreeError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setTree(null);
          setTreeError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setTreeLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [repoId, treePath]);

  useEffect(() => {
    let cancelled = false;

    if (!selectedFilePath) {
      setBlob(null);
      setBlobError(null);
      setBlobLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setBlobLoading(true);

    fetchJson<BlobResponse>(`/api/v1/repos/${repoId}/blob?path=${encodeURIComponent(selectedFilePath)}`)
      .then((data) => {
        if (!cancelled) {
          setBlob(data);
          setBlobError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setBlob(null);
          setBlobError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setBlobLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [repoId, selectedFilePath]);

  const parentPath = useMemo(() => {
    if (!treePath) return null;
    const segments = treePath.split('/');
    segments.pop();
    return segments.join('/');
  }, [treePath]);

  const openEntry = (entry: BrowseEntry) => {
    if (entry.kind === 'dir') {
      setTreePath(entry.path);
      setSelectedFilePath(null);
      setBlob(null);
      setBlobError(null);
      return;
    }

    setSelectedFilePath(entry.path);
  };

  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'minmax(260px, 1fr) minmax(0, 2fr)', gap: 12 }}>
      <section style={browseSectionStyle}>
        <div style={browseSectionHeaderStyle}>
          <div>
            <div style={browseSectionTitleStyle}>Files</div>
            <div style={browseSectionMetaStyle}>Current path: {treePath || '/'}</div>
          </div>
          {parentPath !== null ? (
            <button type="button" style={secondaryButtonStyle} onClick={() => setTreePath(parentPath)}>
              Up
            </button>
          ) : null}
        </div>

        {treeLoading ? <div>Loading files…</div> : null}
        {!treeLoading && treeError ? <div>Unable to load files: {treeError}</div> : null}

        {!treeLoading && !treeError && tree ? (
          tree.entries.length > 0 ? (
            <div style={{ display: 'grid', gap: 8 }}>
              {tree.entries.map((entry) => (
                <button
                  key={entry.path}
                  type="button"
                  onClick={() => openEntry(entry)}
                  style={{
                    ...entryButtonStyle,
                    fontWeight: selectedFilePath === entry.path ? 700 : 500,
                    background: selectedFilePath === entry.path ? '#ddf4ff' : '#fff',
                  }}
                >
                  <span>{entry.kind === 'dir' ? '📁' : '📄'}</span>
                  <span>{entry.kind === 'dir' ? `${entry.name}/` : entry.name}</span>
                </button>
              ))}
            </div>
          ) : (
            <div>This directory is empty.</div>
          )
        ) : null}
      </section>

      <section style={browseSectionStyle}>
        <div style={browseSectionHeaderStyle}>
          <div>
            <div style={browseSectionTitleStyle}>Source</div>
            <div style={browseSectionMetaStyle}>{blob?.path ?? selectedFilePath ?? 'Select a file to inspect its contents.'}</div>
          </div>
          {blob ? <div style={browseSectionMetaStyle}>{blob.size_bytes} bytes</div> : null}
        </div>

        {blobLoading ? <div>Loading source…</div> : null}
        {!blobLoading && blobError ? <div>Unable to load source: {blobError}</div> : null}
        {!blobLoading && !blobError && blob ? (
          <pre
            style={{
              margin: 0,
              padding: 16,
              borderRadius: 12,
              background: '#0d1117',
              color: '#e6edf3',
              overflowX: 'auto',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            }}
          >
            {blob.content}
          </pre>
        ) : null}
        {!blobLoading && !blobError && !blob ? <div style={{ color: '#57606a' }}>Select a file to inspect its contents.</div> : null}
      </section>
    </div>
  );
}

function Panel({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
}) {
  return (
    <section
      style={{
        background: '#f6f8fa',
        border: '1px solid #d0d7de',
        borderRadius: 16,
        padding: 24,
      }}
    >
      <div style={{ marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 24 }}>{title}</h2>
        {subtitle ? <p style={{ margin: '8px 0 0', color: '#57606a' }}>{subtitle}</p> : null}
      </div>
      {children}
    </section>
  );
}

function Detail({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div style={{ padding: 16, borderRadius: 12, background: '#fff', border: '1px solid #d8dee4' }}>
      <div style={{ fontSize: 12, color: '#57606a', marginBottom: 8, textTransform: 'uppercase' }}>{label}</div>
      <div style={{ fontSize: 16, fontWeight: 600 }}>{value}</div>
    </div>
  );
}

function shortCommitId(commitId: string) {
  return commitId.slice(0, 7);
}

const detailGridStyle: CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
  gap: 12,
};

const browseSectionStyle: CSSProperties = {
  background: '#fff',
  border: '1px solid #d8dee4',
  borderRadius: 12,
  padding: 16,
  minHeight: 260,
};

const detailCardStyle: CSSProperties = {
  padding: 16,
  borderRadius: 12,
  background: '#fff',
  border: '1px solid #d8dee4',
};

const browseSectionHeaderStyle: CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  gap: 12,
  marginBottom: 12,
};

const browseSectionTitleStyle: CSSProperties = {
  fontSize: 16,
  fontWeight: 700,
};

const browseSectionMetaStyle: CSSProperties = {
  color: '#57606a',
  fontSize: 13,
  marginTop: 4,
};

const entryButtonStyle: CSSProperties = {
  width: '100%',
  textAlign: 'left',
  padding: '10px 12px',
  borderRadius: 10,
  border: '1px solid #d8dee4',
  background: '#fff',
  color: '#1f2328',
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  cursor: 'pointer',
};

const secondaryButtonStyle: CSSProperties = {
  padding: '8px 12px',
  borderRadius: 10,
  border: '1px solid #d0d7de',
  background: '#fff',
  cursor: 'pointer',
  color: '#1f2328',
  fontWeight: 600,
};

const primaryButtonStyle: CSSProperties = {
  ...secondaryButtonStyle,
  background: '#0969da',
  border: '1px solid #0969da',
  color: '#fff',
};

const searchFormGridStyle: CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
  gap: 12,
};

const fieldLabelStyle: CSSProperties = {
  display: 'grid',
  gap: 8,
  fontSize: 14,
  fontWeight: 600,
};

const inputStyle: CSSProperties = {
  width: '100%',
  padding: '10px 12px',
  borderRadius: 10,
  border: '1px solid #d0d7de',
  background: '#fff',
  color: '#1f2328',
  font: 'inherit',
  boxSizing: 'border-box',
};

const searchResultCardStyle: CSSProperties = {
  padding: 16,
  border: '1px solid #d8dee4',
  borderRadius: 12,
  background: '#fff',
};

const searchMetaBadgeStyle: CSSProperties = {
  display: 'inline-block',
  padding: '4px 10px',
  borderRadius: 999,
  background: '#ddf4ff',
  color: '#0969da',
  fontSize: 12,
  fontWeight: 700,
};

const searchLineStyle: CSSProperties = {
  margin: 0,
  fontFamily: 'ui-monospace, SFMono-Regular, SFMono-Regular, Consolas, monospace',
  fontSize: 13,
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  color: '#1f2328',
};

export function App() {
  const hash = useHashLocation();

  const route = useMemo(() => {
    const match = hash.match(/^#\/repos\/([^/]+)$/);
    if (match) {
      return { kind: 'repo' as const, repoId: decodeURIComponent(match[1]) };
    }
    return { kind: 'home' as const };
  }, [hash]);

  return (
    <main
      style={{
        fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif',
        padding: 24,
        background: '#ffffff',
        color: '#1f2328',
        minHeight: '100vh',
      }}
    >
      <header style={{ marginBottom: 24 }}>
        <h1 style={{ margin: 0, fontSize: 32 }}>sourcebot-rewrite</h1>
        <p style={{ color: '#57606a', marginTop: 8 }}>
          Clean-room code intelligence workspace: repository inventory, sync state, and API-backed detail views.
        </p>
      </header>

      {route.kind === 'repo' ? <RepoDetailPage repoId={route.repoId} /> : <RepoListPage />}
    </main>
  );
}
