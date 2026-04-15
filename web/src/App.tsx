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
  kind: 'tree' | 'blob';
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

  if (loading) return <Panel title="Repositories">Loading repositories…</Panel>;
  if (error) return <Panel title="Repositories">Failed to load: {error}</Panel>;

  return (
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
    if (entry.kind === 'tree') {
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
                  <span>{entry.kind === 'tree' ? '📁' : '📄'}</span>
                  <span>{entry.kind === 'tree' ? `${entry.name}/` : entry.name}</span>
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
