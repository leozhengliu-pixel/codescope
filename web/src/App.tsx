import { useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent, type ReactNode } from 'react';

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

type DefinitionRange = {
  start_line: number;
  end_line: number;
};

type DefinitionResult = {
  path: string;
  name: string;
  kind: string;
  range: DefinitionRange;
  browse_url: string;
};

type ReferenceResult = {
  path: string;
  line_number: number;
  line: string;
  browse_url: string;
};

type DefinitionsResponse =
  | {
      status: 'supported';
      repo_id: string;
      path: string;
      revision: string | null;
      symbol: string;
      definitions: DefinitionResult[];
    }
  | {
      status: 'unsupported';
      repo_id: string;
      path: string;
      revision: string | null;
      symbol: string;
      capability: string;
      definitions: DefinitionResult[];
    };

type ReferencesResponse =
  | {
      status: 'supported';
      repo_id: string;
      path: string;
      revision: string | null;
      symbol: string;
      references: ReferenceResult[];
    }
  | {
      status: 'unsupported';
      repo_id: string;
      path: string;
      revision: string | null;
      symbol: string;
      capability: string;
      references: ReferenceResult[];
    };

type NavigationMode = 'definitions' | 'references';

type NavigationState = {
  mode: NavigationMode;
  symbol: string;
  revision: string | null;
  capability: string | null;
  definitions: DefinitionResult[];
  references: ReferenceResult[];
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

type CommitDiffFile = {
  path: string;
  change_type: 'added' | 'modified' | 'deleted' | 'renamed';
  old_path: string | null;
  additions: number;
  deletions: number;
  patch: string | null;
};

type CommitDiffResponse = {
  repo_id: string;
  commit_id: string;
  files: CommitDiffFile[];
};

type AuthConnectionConfig =
  | {
      provider: 'github' | 'gitlab' | 'gitea' | 'gerrit' | 'bitbucket' | 'azure_devops' | 'generic_git';
      base_url: string;
    }
  | {
      provider: 'local';
      repo_path: string;
    };

type AuthConnectionKind = AuthConnectionConfig['provider'];

type AuthConnection = {
  id: string;
  name: string;
  kind: string;
  config?: AuthConnectionConfig;
};

type RepositorySyncJobStatus = 'queued' | 'running' | 'succeeded' | 'failed';

type RepositorySyncJob = {
  id: string;
  organization_id: string;
  repository_id: string;
  connection_id: string;
  status: RepositorySyncJobStatus;
  queued_at: string;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
};

type CreateAuthConnectionRequest = {
  name: string;
  kind: AuthConnectionKind;
  config?: AuthConnectionConfig;
};

type EditConnectionDraft = {
  connectionId: string;
  name: string;
  configValue: string;
};

type LocalImportState = {
  path: string;
  importing: boolean;
  error: string | null;
  result: RepoDetail | null;
};

const authConnectionKindOptions: AuthConnectionKind[] = [
  'github',
  'gitlab',
  'gitea',
  'gerrit',
  'bitbucket',
  'azure_devops',
  'generic_git',
  'local',
];

function useHashLocation() {
  const [hash, setHash] = useState(() => window.location.hash || '#/');

  useEffect(() => {
    const onChange = () => setHash(window.location.hash || '#/');
    window.addEventListener('hashchange', onChange);
    return () => window.removeEventListener('hashchange', onChange);
  }, []);

  return hash;
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = init ? await fetch(path, init) : await fetch(path);
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status}`);
  }
  return (await response.json()) as T;
}

function sharedStatusBadgeStyle(color: string): CSSProperties {
  return {
    display: 'inline-block',
    padding: '2px 10px',
    borderRadius: 999,
    background: `${color}18`,
    color,
    fontSize: 12,
    fontWeight: 600,
    textTransform: 'uppercase',
  };
}

function StatusBadge({ state }: { state: SyncState }) {
  const colors: Record<SyncState, string> = {
    pending: '#9a6700',
    ready: '#1a7f37',
    error: '#cf222e',
  };

  return <span style={sharedStatusBadgeStyle(colors[state])}>{state}</span>;
}

function RepositorySyncJobStatusBadge({ status }: { status: RepositorySyncJobStatus }) {
  const colors: Record<RepositorySyncJobStatus, string> = {
    queued: '#9a6700',
    running: '#0969da',
    succeeded: '#1a7f37',
    failed: '#cf222e',
  };

  return <span style={sharedStatusBadgeStyle(colors[status])}>{status}</span>;
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
  const [commitDiff, setCommitDiff] = useState<CommitDiffResponse | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setCommitsLoading(true);
    setSelectedCommitId(null);
    setCommitDetail(null);
    setCommitError(null);
    setCommitLoading(false);
    setCommitDiff(null);
    setCommitDiffError(null);
    setCommitDiffLoading(false);

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

  useEffect(() => {
    let cancelled = false;

    if (!selectedCommitId) {
      setCommitDiff(null);
      setCommitDiffError(null);
      setCommitDiffLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setCommitDiffLoading(true);
    setCommitDiffError(null);
    setCommitDiff(null);

    fetchJson<CommitDiffResponse>(`/api/v1/repos/${repoId}/commits/${encodeURIComponent(selectedCommitId)}/diff`)
      .then((data) => {
        if (!cancelled) {
          setCommitDiff(data);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setCommitDiff(null);
          setCommitDiffError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setCommitDiffLoading(false);
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
            <div style={detailCardStyle}>
              <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>Changed files</div>
              {commitDiffLoading ? <div>Loading diff…</div> : null}
              {!commitDiffLoading && commitDiffError ? <div>Unable to load diff: {commitDiffError}</div> : null}
              {!commitDiffLoading && !commitDiffError && commitDiff ? (
                commitDiff.files.length > 0 ? (
                  <div style={{ display: 'grid', gap: 12 }}>
                    {commitDiff.files.map((file, index) => (
                      <div key={`${file.path}:${index}`} style={diffFileCardStyle}>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, alignItems: 'center' }}>
                          <div style={{ fontWeight: 700, overflowWrap: 'anywhere' }}>
                            {file.change_type === 'renamed' && file.old_path ? `${file.old_path} → ${file.path}` : file.path}
                          </div>
                          <span style={diffTypeBadgeStyle(file.change_type)}>{file.change_type}</span>
                          <span style={diffStatAdditionsStyle}>+{file.additions}</span>
                          <span style={diffStatDeletionsStyle}>-{file.deletions}</span>
                        </div>
                        {file.patch ? (
                          <pre style={diffPatchStyle}>{file.patch}</pre>
                        ) : (
                          <div style={browseSectionMetaStyle}>Binary file or patch unavailable.</div>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div style={{ color: '#57606a' }}>No changed files reported for this commit.</div>
                )
              ) : null}
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

function pathDirectory(path: string) {
  const segments = path.split('/');
  segments.pop();
  return segments.join('/');
}

function parseBrowseUrl(browseUrl: string) {
  const url = new URL(browseUrl, window.location.origin);
  const match = url.pathname.match(/^\/api\/v1\/repos\/([^/]+)\/blob$/);
  if (!match) {
    return null;
  }

  const path = url.searchParams.get('path');
  if (!path) {
    return null;
  }

  return {
    repoId: decodeURIComponent(match[1]),
    path,
    revision: url.searchParams.get('revision'),
    line: url.hash.startsWith('#L') ? Number(url.hash.slice(2)) || null : null,
  };
}

function BrowsePanel({ repoId }: { repoId: string }) {
  const [treePath, setTreePath] = useState('');
  const [tree, setTree] = useState<TreeResponse | null>(null);
  const [treeLoading, setTreeLoading] = useState(true);
  const [treeError, setTreeError] = useState<string | null>(null);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [selectedRevision, setSelectedRevision] = useState<string | null>(null);
  const [blob, setBlob] = useState<BlobResponse | null>(null);
  const [blobLoading, setBlobLoading] = useState(false);
  const [blobError, setBlobError] = useState<string | null>(null);
  const [symbol, setSymbol] = useState('');
  const [navigationLoading, setNavigationLoading] = useState(false);
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const [navigationState, setNavigationState] = useState<NavigationState | null>(null);
  const navigationRequestId = useRef(0);

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

    const params = new URLSearchParams({ path: selectedFilePath });
    if (selectedRevision) {
      params.set('revision', selectedRevision);
    }

    fetchJson<BlobResponse>(`/api/v1/repos/${repoId}/blob?${params.toString()}`)
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
  }, [repoId, selectedFilePath, selectedRevision]);

  useEffect(() => {
    navigationRequestId.current += 1;
    setSelectedRevision(null);
    setSymbol('');
    setNavigationLoading(false);
    setNavigationError(null);
    setNavigationState(null);
  }, [repoId]);

  const parentPath = useMemo(() => {
    if (!treePath) return null;
    return pathDirectory(treePath);
  }, [treePath]);

  const openEntry = (entry: BrowseEntry) => {
    navigationRequestId.current += 1;
    setNavigationLoading(false);
    if (entry.kind === 'dir') {
      setTreePath(entry.path);
      setSelectedFilePath(null);
      setSelectedRevision(null);
      setBlob(null);
      setBlobError(null);
      setSymbol('');
      setNavigationError(null);
      setNavigationState(null);
      return;
    }

    setSelectedFilePath(entry.path);
    setSelectedRevision(null);
    setSymbol('');
    setNavigationError(null);
    setNavigationState(null);
  };

  const openBrowseTarget = (browseUrl: string) => {
    const target = parseBrowseUrl(browseUrl);
    if (!target || target.repoId !== repoId) {
      return;
    }

    navigationRequestId.current += 1;
    setNavigationLoading(false);
    setTreePath(pathDirectory(target.path));
    setSelectedFilePath(target.path);
    setSelectedRevision(target.revision);
    setNavigationError(null);
  };

  const runNavigation = async (mode: NavigationMode) => {
    const trimmedSymbol = symbol.trim();
    if (!selectedFilePath || !trimmedSymbol) {
      return;
    }

    const requestId = navigationRequestId.current + 1;
    navigationRequestId.current = requestId;
    setNavigationLoading(true);
    setNavigationError(null);

    try {
      const params = new URLSearchParams({ path: selectedFilePath, symbol: trimmedSymbol });
      if (selectedRevision) {
        params.set('revision', selectedRevision);
      }

      if (mode === 'definitions') {
        const data = await fetchJson<DefinitionsResponse>(`/api/v1/repos/${repoId}/definitions?${params.toString()}`);
        if (navigationRequestId.current !== requestId) {
          return;
        }

        setNavigationState({
          mode,
          symbol: data.symbol,
          revision: data.revision,
          capability: data.status === 'unsupported' ? data.capability : null,
          definitions: data.definitions,
          references: [],
        });
      } else {
        const data = await fetchJson<ReferencesResponse>(`/api/v1/repos/${repoId}/references?${params.toString()}`);
        if (navigationRequestId.current !== requestId) {
          return;
        }

        setNavigationState({
          mode,
          symbol: data.symbol,
          revision: data.revision,
          capability: data.status === 'unsupported' ? data.capability : null,
          definitions: [],
          references: data.references,
        });
      }
    } catch (err) {
      if (navigationRequestId.current !== requestId) {
        return;
      }

      setNavigationState(null);
      setNavigationError((err as Error).message);
    } finally {
      if (navigationRequestId.current === requestId) {
        setNavigationLoading(false);
      }
    }
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
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4 }}>
            {blob ? <div style={browseSectionMetaStyle}>{blob.size_bytes} bytes</div> : null}
            {selectedRevision ? <div style={browseSectionMetaStyle}>Viewing revision: {selectedRevision}</div> : null}
          </div>
        </div>

        {selectedFilePath ? (
          <div
            style={{
              display: 'grid',
              gap: 10,
              marginBottom: 16,
              padding: 12,
              borderRadius: 12,
              border: '1px solid #d8dee4',
              background: '#f6f8fa',
            }}
          >
            <label style={{ display: 'grid', gap: 6, fontSize: 14, fontWeight: 600 }}>
              <span>Symbol token</span>
              <input
                value={symbol}
                onChange={(event) => setSymbol(event.target.value)}
                placeholder="Enter a symbol for this file"
                style={{
                  padding: '10px 12px',
                  borderRadius: 10,
                  border: '1px solid #d0d7de',
                  font: 'inherit',
                  color: '#1f2328',
                  background: '#fff',
                }}
              />
            </label>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              <button
                type="button"
                style={secondaryButtonStyle}
                disabled={navigationLoading || symbol.trim().length === 0}
                onClick={() => void runNavigation('definitions')}
              >
                Find definitions
              </button>
              <button
                type="button"
                style={secondaryButtonStyle}
                disabled={navigationLoading || symbol.trim().length === 0}
                onClick={() => void runNavigation('references')}
              >
                Find references
              </button>
            </div>
            {navigationLoading ? <div style={browseSectionMetaStyle}>Loading code navigation…</div> : null}
            {navigationError ? <div>Unable to load navigation results: {navigationError}</div> : null}
            {navigationState ? (
              <div style={{ display: 'grid', gap: 10 }}>
                <div style={{ display: 'grid', gap: 4 }}>
                  <div style={{ fontWeight: 700 }}>
                    {navigationState.mode === 'definitions' ? 'Definition results' : 'Reference results'}
                  </div>
                  {navigationState.revision ? <div style={browseSectionMetaStyle}>Revision: {navigationState.revision}</div> : null}
                </div>
                {navigationState.capability ? <div style={browseSectionMetaStyle}>{navigationState.capability}</div> : null}
                {!navigationState.capability && navigationState.mode === 'definitions' && navigationState.definitions.length === 0 ? (
                  <div style={browseSectionMetaStyle}>No definitions found for “{navigationState.symbol}”.</div>
                ) : null}
                {!navigationState.capability && navigationState.mode === 'references' && navigationState.references.length === 0 ? (
                  <div style={browseSectionMetaStyle}>No references found for “{navigationState.symbol}”.</div>
                ) : null}
                {!navigationState.capability && navigationState.mode === 'definitions' && navigationState.definitions.length > 0 ? (
                  <div style={{ display: 'grid', gap: 8 }}>
                    {navigationState.definitions.map((definition, index) => (
                      <button
                        key={`${definition.path}-${definition.range.start_line}-${index}`}
                        type="button"
                        style={{ ...entryButtonStyle, alignItems: 'flex-start', flexDirection: 'column' }}
                        onClick={() => openBrowseTarget(definition.browse_url)}
                      >
                        <span style={{ fontWeight: 700 }}>{definition.name}</span>
                        <span>{definition.path}</span>
                        <span style={browseSectionMetaStyle}>{definition.kind}</span>
                        <span style={browseSectionMetaStyle}>Lines {definition.range.start_line}–{definition.range.end_line}</span>
                      </button>
                    ))}
                  </div>
                ) : null}
                {!navigationState.capability && navigationState.mode === 'references' && navigationState.references.length > 0 ? (
                  <div style={{ display: 'grid', gap: 8 }}>
                    {navigationState.references.map((reference, index) => (
                      <button
                        key={`${reference.path}-${reference.line_number}-${index}`}
                        type="button"
                        style={{ ...entryButtonStyle, alignItems: 'flex-start', flexDirection: 'column' }}
                        onClick={() => openBrowseTarget(reference.browse_url)}
                      >
                        <span style={{ fontWeight: 700 }}>{reference.path}</span>
                        <span style={browseSectionMetaStyle}>Line {reference.line_number}</span>
                        <span style={{ ...browseSectionMetaStyle, whiteSpace: 'normal' }}>{reference.line}</span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        ) : null}

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

function connectionConfigSummary(connection: AuthConnection): string | null {
  if (!connection.config) {
    return null;
  }

  if ('base_url' in connection.config) {
    return `Base URL: ${connection.config.base_url}`;
  }

  return `Repo path: ${connection.config.repo_path}`;
}

function connectionEditFieldLabel(connection: AuthConnection): 'Edit base URL' | 'Edit repo path' {
  return connection.kind === 'local' ? 'Edit repo path' : 'Edit base URL';
}

function localConnectionRepoPath(connection: AuthConnection) {
  if (connection.kind === 'local' && connection.config && 'repo_path' in connection.config) {
    return connection.config.repo_path;
  }

  return '';
}

function initialLocalImportState(connection: AuthConnection): LocalImportState {
  return {
    path: localConnectionRepoPath(connection),
    importing: false,
    error: null,
    result: null,
  };
}

function genericGitConnectionBaseUrl(connection: AuthConnection) {
  if (!(connection.kind === 'generic_git' && connection.config && 'base_url' in connection.config)) {
    return '';
  }

  try {
    const parsedUrl = new URL(connection.config.base_url);
    if (parsedUrl.protocol === 'http:' || parsedUrl.protocol === 'https:') {
      return parsedUrl.toString();
    }
  } catch {
    return '';
  }

  return '';
}

function connectionDiscoveryStatus(connection: AuthConnection): string[] | null {
  if (connection.kind === 'generic_git') {
    return ['Repository discovery is not available yet for generic Git connections.'];
  }

  if (connection.kind === 'local') {
    return [
      'Import one repository path at a time from this local root.',
      'Recursive local enumeration is not available yet.',
    ];
  }

  return null;
}

function buildConnectionUpdateRequest(connection: AuthConnection, draft: EditConnectionDraft): CreateAuthConnectionRequest {
  const kind = connection.kind as AuthConnectionKind;

  return {
    name: draft.name,
    kind,
    config:
      kind === 'local'
        ? {
            provider: 'local',
            repo_path: draft.configValue,
          }
        : {
            provider: kind,
            base_url: draft.configValue,
          },
  };
}

function repositorySyncJobActivityTimestamp(syncJob: RepositorySyncJob) {
  return Date.parse(syncJob.finished_at ?? syncJob.started_at ?? syncJob.queued_at);
}

function compareRepositorySyncJobs(left: RepositorySyncJob, right: RepositorySyncJob) {
  const queuedAtDifference = Date.parse(right.queued_at) - Date.parse(left.queued_at);
  if (queuedAtDifference !== 0) {
    return queuedAtDifference;
  }

  return repositorySyncJobActivityTimestamp(right) - repositorySyncJobActivityTimestamp(left);
}

function compareLatestRepositorySyncJobs(left: RepositorySyncJob, right: RepositorySyncJob) {
  const baseComparison = compareRepositorySyncJobs(left, right);
  if (baseComparison !== 0) {
    return baseComparison;
  }

  const inProgressPriority = (status: RepositorySyncJobStatus) => {
    if (status === 'running') {
      return 2;
    }
    if (status === 'queued') {
      return 1;
    }
    return 0;
  };

  return inProgressPriority(right.status) - inProgressPriority(left.status);
}

function repositorySyncJobsByConnectionId(syncJobs: RepositorySyncJob[]) {
  return syncJobs.reduce<Map<string, RepositorySyncJob[]>>((jobsByConnectionId, syncJob) => {
    const existingJobs = jobsByConnectionId.get(syncJob.connection_id) ?? [];
    existingJobs.push(syncJob);
    existingJobs.sort(compareRepositorySyncJobs);
    jobsByConnectionId.set(syncJob.connection_id, existingJobs);
    return jobsByConnectionId;
  }, new Map<string, RepositorySyncJob[]>());
}

function latestRepositorySyncJob(syncJobs: RepositorySyncJob[]) {
  return syncJobs.reduce<RepositorySyncJob | null>((latestSyncJob, syncJob) => {
    if (latestSyncJob === null) {
      return syncJob;
    }

    return compareLatestRepositorySyncJobs(syncJob, latestSyncJob) < 0 ? syncJob : latestSyncJob;
  }, null);
}

function SettingsConnectionsPage() {
  const [connections, setConnections] = useState<AuthConnection[]>([]);
  const [syncJobs, setSyncJobs] = useState<RepositorySyncJob[]>([]);
  const [syncJobsError, setSyncJobsError] = useState<string | null>(null);
  const [syncJobsLoading, setSyncJobsLoading] = useState(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleteErrorConnectionId, setDeleteErrorConnectionId] = useState<string | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [deletingConnectionId, setDeletingConnectionId] = useState<string | null>(null);
  const [editingConnection, setEditingConnection] = useState<EditConnectionDraft | null>(null);
  const [updatingConnectionId, setUpdatingConnectionId] = useState<string | null>(null);
  const [localImportStates, setLocalImportStates] = useState<Record<string, LocalImportState>>({});
  const [connectionName, setConnectionName] = useState('');
  const [connectionKind, setConnectionKind] = useState<AuthConnectionKind>('github');
  const [baseUrl, setBaseUrl] = useState('');
  const [repoPath, setRepoPath] = useState('');

  useEffect(() => {
    let cancelled = false;

    fetchJson<AuthConnection[]>('/api/v1/auth/connections')
      .then((data) => {
        if (!cancelled) {
          setConnections(data);
          setLocalImportStates((currentStates) => {
            const nextStates: Record<string, LocalImportState> = {};

            for (const connection of data) {
              if (connection.kind !== 'local') {
                continue;
              }

              const existingState = currentStates[connection.id];
              nextStates[connection.id] = existingState
                ? { ...existingState, path: existingState.path || localConnectionRepoPath(connection) }
                : initialLocalImportState(connection);
            }

            return nextStates;
          });
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setConnections([]);
          setError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    fetchJson<RepositorySyncJob[]>('/api/v1/auth/repository-sync-jobs')
      .then((data) => {
        if (!cancelled) {
          setSyncJobs(data);
          setSyncJobsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setSyncJobs([]);
          setSyncJobsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setSyncJobsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const createDisabled = loading || isCreating;
  const deleteDisabled = deletingConnectionId !== null;
  const editControlsDisabled = updatingConnectionId !== null || deletingConnectionId !== null;
  const syncJobsByConnectionId = useMemo(() => repositorySyncJobsByConnectionId(syncJobs), [syncJobs]);

  const handleCreateConnection = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setCreateError(null);
    setDeleteError(null);
    setDeleteErrorConnectionId(null);
    setUpdateError(null);
    setIsCreating(true);

    const request: CreateAuthConnectionRequest = {
      name: connectionName,
      kind: connectionKind,
      config:
        connectionKind === 'local'
          ? {
              provider: 'local',
              repo_path: repoPath,
            }
          : {
              provider: connectionKind,
              base_url: baseUrl,
            },
    };

    try {
      const createdConnection = await fetchJson<AuthConnection>('/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });

      setConnections((currentConnections) => [...currentConnections, createdConnection]);
      if (createdConnection.kind === 'local') {
        setLocalImportStates((currentStates) => ({
          ...currentStates,
          [createdConnection.id]: initialLocalImportState(createdConnection),
        }));
      }
      setConnectionName('');
      setBaseUrl('');
      setRepoPath('');
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setIsCreating(false);
    }
  };

  const handleDeleteConnection = async (connectionId: string) => {
    setDeleteError(null);
    setDeleteErrorConnectionId(null);
    setUpdateError(null);
    setDeletingConnectionId(connectionId);

    try {
      const response = await fetch(`/api/v1/auth/connections/${connectionId}`, {
        method: 'DELETE',
      });

      if (!response.ok) {
        throw new Error(`Request failed: ${response.status}`);
      }

      setConnections((currentConnections) =>
        currentConnections.filter((connection) => connection.id !== connectionId)
      );
      setLocalImportStates((currentStates) => {
        if (!(connectionId in currentStates)) {
          return currentStates;
        }

        const nextStates = { ...currentStates };
        delete nextStates[connectionId];
        return nextStates;
      });
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : 'Unknown error');
      setDeleteErrorConnectionId(connectionId);
    } finally {
      setDeletingConnectionId(null);
    }
  };

  const startEditingConnection = (connection: AuthConnection) => {
    setCreateError(null);
    setDeleteError(null);
    setDeleteErrorConnectionId(null);
    setUpdateError(null);
    setEditingConnection({
      connectionId: connection.id,
      name: connection.name,
      configValue:
        connection.config && 'repo_path' in connection.config
          ? connection.config.repo_path
          : connection.config && 'base_url' in connection.config
            ? connection.config.base_url
            : '',
    });
  };

  const handleUpdateConnection = async (event: FormEvent<HTMLFormElement>, connection: AuthConnection) => {
    event.preventDefault();

    if (!editingConnection || editingConnection.connectionId !== connection.id) {
      return;
    }

    setCreateError(null);
    setDeleteError(null);
    setDeleteErrorConnectionId(null);
    setUpdateError(null);
    setUpdatingConnectionId(connection.id);

    try {
      const updatedConnection = await fetchJson<AuthConnection>(`/api/v1/auth/connections/${connection.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(buildConnectionUpdateRequest(connection, editingConnection)),
      });

      setConnections((currentConnections) =>
        currentConnections.map((currentConnection) =>
          currentConnection.id === updatedConnection.id ? updatedConnection : currentConnection
        )
      );
      if (updatedConnection.kind === 'local') {
        setLocalImportStates((currentStates) => ({
          ...currentStates,
          [updatedConnection.id]: initialLocalImportState(updatedConnection),
        }));
      }
      setEditingConnection(null);
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setUpdatingConnectionId(null);
    }
  };

  const handleLocalImportPathChange = (connection: AuthConnection, path: string) => {
    if (connection.kind !== 'local') {
      return;
    }

    setLocalImportStates((currentStates) => {
      const currentState = currentStates[connection.id] ?? initialLocalImportState(connection);
      const nextPath = path;

      const pathChanged = nextPath !== currentState.path;

      return {
        ...currentStates,
        [connection.id]: {
          ...currentState,
          path: nextPath,
          error: pathChanged ? null : currentState.error,
          result: pathChanged ? null : currentState.result,
        },
      };
    });
  };

  const handleImportLocalRepository = async (event: FormEvent<HTMLFormElement>, connection: AuthConnection) => {
    event.preventDefault();

    if (connection.kind !== 'local') {
      return;
    }

    const importState = localImportStates[connection.id] ?? initialLocalImportState(connection);
    const path = importState.path.trim();

    setLocalImportStates((currentStates) => ({
      ...currentStates,
      [connection.id]: {
        ...(currentStates[connection.id] ?? initialLocalImportState(connection)),
        path,
        importing: true,
        error: null,
        result: null,
      },
    }));

    try {
      const importedRepository = await fetchJson<RepoDetail>('/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: connection.id,
          path,
        }),
      });

      setLocalImportStates((currentStates) => ({
        ...currentStates,
        [connection.id]: {
          ...(currentStates[connection.id] ?? initialLocalImportState(connection)),
          path,
          importing: false,
          error: null,
          result: importedRepository,
        },
      }));
    } catch (err) {
      setLocalImportStates((currentStates) => ({
        ...currentStates,
        [connection.id]: {
          ...(currentStates[connection.id] ?? initialLocalImportState(connection)),
          path,
          importing: false,
          error: err instanceof Error ? err.message : 'Unknown error',
          result: null,
        },
      }));
    }
  };

  return (
    <Panel
      title="Authenticated connections"
      subtitle="Create, edit, and remove authenticated connections from the existing authenticated API while richer sync controls remain out of scope."
    >
      {!loading && !error ? (
        <form onSubmit={handleCreateConnection} style={{ display: 'grid', gap: 12, marginBottom: 20 }}>
          <div style={detailGridStyle}>
            <label style={fieldLabelStyle}>
              <span>Connection name</span>
              <input
                value={connectionName}
                onChange={(event) => setConnectionName(event.target.value)}
                style={inputStyle}
                disabled={createDisabled}
              />
            </label>

            <label style={fieldLabelStyle}>
              <span>Connection kind</span>
              <select
                value={connectionKind}
                onChange={(event) => setConnectionKind(event.target.value as AuthConnectionKind)}
                style={inputStyle}
                disabled={createDisabled}
              >
                {authConnectionKindOptions.map((kind) => (
                  <option key={kind} value={kind}>
                    {kind}
                  </option>
                ))}
              </select>
            </label>

            {connectionKind === 'local' ? (
              <label style={fieldLabelStyle}>
                <span>Repo path</span>
                <input
                  value={repoPath}
                  onChange={(event) => setRepoPath(event.target.value)}
                  style={inputStyle}
                  disabled={createDisabled}
                />
              </label>
            ) : (
              <label style={fieldLabelStyle}>
                <span>Base URL</span>
                <input
                  value={baseUrl}
                  onChange={(event) => setBaseUrl(event.target.value)}
                  style={inputStyle}
                  disabled={createDisabled}
                />
              </label>
            )}
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
            <button type="submit" style={primaryButtonStyle} disabled={createDisabled}>
              {isCreating ? 'Creating…' : 'Create connection'}
            </button>
            {createError ? <div>Failed to create connection: {createError}</div> : null}
          </div>
        </form>
      ) : null}

      {loading ? <div>Loading connections…</div> : null}
      {!loading && error ? <div>Failed to load connections: {error}</div> : null}
      {!loading && !error && connections.length === 0 ? <div>No authenticated connections are available.</div> : null}
      {!loading && !error && connections.length > 0 ? (
        <div style={{ display: 'grid', gap: 12 }}>
          {connections.map((connection) => {
            const configSummary = connectionConfigSummary(connection);
            const discoveryStatus = connectionDiscoveryStatus(connection);
            const genericGitQuickOpenUrl = genericGitConnectionBaseUrl(connection);
            const isEditing = editingConnection?.connectionId === connection.id;
            const isUpdating = updatingConnectionId === connection.id;
            const connectionSyncJobs = syncJobsByConnectionId.get(connection.id) ?? [];
            const latestConnectionSyncJob = latestRepositorySyncJob(connectionSyncJobs);
            const localImportState = connection.kind === 'local' ? localImportStates[connection.id] ?? initialLocalImportState(connection) : null;
            const localImportRootPath = connection.kind === 'local' ? localConnectionRepoPath(connection) : '';
            const showLocalImportReset = connection.kind === 'local' && localImportState !== null && localImportState.path !== localImportRootPath;
            const importControlsDisabled = localImportState?.importing ?? false;
            const importFormDisabled = importControlsDisabled || isUpdating || deletingConnectionId === connection.id;
            const editFormDisabled = isUpdating || importControlsDisabled;

            return (
              <article key={connection.id} style={detailCardStyle}>
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                  <div style={{ display: 'grid', gap: 6 }}>
                    <div style={{ fontSize: 18, fontWeight: 700 }}>{connection.name}</div>
                    <div style={{ color: '#57606a' }}>Kind: {connection.kind}</div>
                    <div style={{ color: '#57606a' }}>Connection id: {connection.id}</div>
                    {configSummary ? <div style={{ color: '#57606a' }}>{configSummary}</div> : null}
                  </div>
                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                    <button
                      type="button"
                      style={secondaryButtonStyle}
                      disabled={editControlsDisabled || importControlsDisabled}
                      onClick={() => {
                        startEditingConnection(connection);
                      }}
                    >
                      {isEditing ? `Editing ${connection.name}` : `Edit ${connection.name}`}
                    </button>
                    <button
                      type="button"
                      style={secondaryButtonStyle}
                      disabled={deleteDisabled || updatingConnectionId !== null || importControlsDisabled}
                      onClick={() => {
                        void handleDeleteConnection(connection.id);
                      }}
                    >
                      {deletingConnectionId === connection.id ? 'Deleting…' : `Delete ${connection.name}`}
                    </button>
                  </div>
                </div>

                {deleteError && deleteErrorConnectionId === connection.id ? (
                  <div style={{ marginTop: 16, color: '#cf222e' }}>Failed to delete connection: {deleteError}</div>
                ) : null}

                {isEditing && editingConnection ? (
                  <form
                    onSubmit={(event) => {
                      void handleUpdateConnection(event, connection);
                    }}
                    style={{ display: 'grid', gap: 12, marginTop: 16 }}
                  >
                    <div style={detailGridStyle}>
                      <label style={fieldLabelStyle}>
                        <span>Edit connection name</span>
                        <input
                          value={editingConnection.name}
                          onChange={(event) =>
                            setEditingConnection((currentDraft) =>
                              currentDraft && currentDraft.connectionId === connection.id
                                ? { ...currentDraft, name: event.target.value }
                                : currentDraft
                            )
                          }
                          style={inputStyle}
                          disabled={editFormDisabled}
                        />
                      </label>

                      <label style={fieldLabelStyle}>
                        <span>{connectionEditFieldLabel(connection)}</span>
                        <input
                          value={editingConnection.configValue}
                          onChange={(event) =>
                            setEditingConnection((currentDraft) =>
                              currentDraft && currentDraft.connectionId === connection.id
                                ? { ...currentDraft, configValue: event.target.value }
                                : currentDraft
                            )
                          }
                          style={inputStyle}
                          disabled={editFormDisabled}
                        />
                      </label>
                    </div>

                    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                      <button type="submit" style={primaryButtonStyle} disabled={editFormDisabled}>
                        {isUpdating ? 'Saving…' : 'Save changes'}
                      </button>
                      <button
                        type="button"
                        style={secondaryButtonStyle}
                        disabled={editFormDisabled}
                        onClick={() => {
                          setEditingConnection(null);
                          setUpdateError(null);
                        }}
                      >
                        Cancel
                      </button>
                    </div>
                    {updateError ? <div>Failed to update connection: {updateError}</div> : null}
                  </form>
                ) : null}

                {discoveryStatus ? (
                  <section style={{ marginTop: 16, paddingTop: 16, borderTop: '1px solid #d8dee4', display: 'grid', gap: 8 }}>
                    <div style={{ fontWeight: 600 }}>Discovery status</div>
                    {discoveryStatus.map((message) => (
                      <div key={message} style={{ color: '#57606a' }}>
                        {message}
                      </div>
                    ))}
                    {genericGitQuickOpenUrl ? (
                      <a
                        href={genericGitQuickOpenUrl}
                        target="_blank"
                        rel="noreferrer"
                        style={{ color: '#0969da', fontWeight: 600 }}
                      >
                        Open host for manual discovery
                      </a>
                    ) : null}
                  </section>
                ) : null}

                {connection.kind === 'local' && localImportState ? (
                  <section style={{ marginTop: 16, paddingTop: 16, borderTop: '1px solid #d8dee4', display: 'grid', gap: 12 }}>
                    <div style={{ fontWeight: 600 }}>Import repository</div>
                    <form
                      onSubmit={(event) => {
                        void handleImportLocalRepository(event, connection);
                      }}
                      style={{ display: 'grid', gap: 12 }}
                    >
                      <label style={fieldLabelStyle}>
                        <span>Repository path</span>
                        <input
                          value={localImportState.path}
                          onChange={(event) => {
                            handleLocalImportPathChange(connection, event.target.value);
                          }}
                          style={inputStyle}
                          disabled={importFormDisabled}
                        />
                      </label>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
                        <button
                          type="submit"
                          style={primaryButtonStyle}
                          disabled={importFormDisabled || localImportState.path.trim().length === 0}
                        >
                          {localImportState.importing ? 'Importing…' : 'Import repository'}
                        </button>
                        {showLocalImportReset ? (
                          <button
                            type="button"
                            style={secondaryButtonStyle}
                            disabled={importFormDisabled}
                            onClick={() => {
                              handleLocalImportPathChange(connection, localImportRootPath);
                            }}
                          >
                            Reset to local root
                          </button>
                        ) : null}
                        {localImportState.error ? (
                          <div style={{ color: '#cf222e' }}>Failed to import repository: {localImportState.error}</div>
                        ) : null}
                      </div>
                    </form>
                    {localImportState.result ? (
                      <div style={{ padding: 12, borderRadius: 10, border: '1px solid #d8dee4', background: '#f6f8fa', display: 'grid', gap: 8 }}>
                        <div>Imported repository: {localImportState.result.repository.name}</div>
                        <div>Repository id: {localImportState.result.repository.id}</div>
                        <div>Default branch: {localImportState.result.repository.default_branch}</div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                          <span>Sync state</span>
                          <StatusBadge state={localImportState.result.repository.sync_state} />
                        </div>
                        <a
                          href={`#/repos/${encodeURIComponent(localImportState.result.repository.id)}`}
                          style={{ color: '#0969da', fontWeight: 600 }}
                        >
                          Open repository detail
                        </a>
                      </div>
                    ) : null}
                  </section>
                ) : null}

                <section style={{ marginTop: 16, paddingTop: 16, borderTop: '1px solid #d8dee4', display: 'grid', gap: 12 }}>
                  <div style={{ fontWeight: 600 }}>Repository sync history</div>
                  {syncJobsError ? (
                    <div style={{ color: '#cf222e' }}>Failed to load repository sync history: {syncJobsError}</div>
                  ) : null}
                  {!syncJobsError && syncJobsLoading ? (
                    <div style={{ color: '#57606a', fontSize: 14 }}>Loading repository sync history…</div>
                  ) : null}
                  {!syncJobsError && !syncJobsLoading && latestConnectionSyncJob ? (
                    <div
                      aria-label={`Latest sync summary for ${connection.name}`}
                      style={{ color: '#57606a', fontSize: 14, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}
                    >
                      <span>Latest sync:</span>
                      <RepositorySyncJobStatusBadge status={latestConnectionSyncJob.status} />
                      <span>
                        {latestConnectionSyncJob.repository_id} · {latestConnectionSyncJob.queued_at}
                      </span>
                    </div>
                  ) : null}
                  {!syncJobsError && !syncJobsLoading && connectionSyncJobs.length === 0 ? (
                    <div style={{ color: '#57606a', fontSize: 14 }}>No repository sync jobs found for this connection.</div>
                  ) : null}
                  {!syncJobsError && connectionSyncJobs.length > 0 ? (
                    <div style={{ display: 'grid', gap: 8 }}>
                      {connectionSyncJobs.map((syncJob) => (
                        <div
                          key={syncJob.id}
                          aria-label={`Repository sync history row for ${syncJob.repository_id}`}
                          style={{ padding: 12, borderRadius: 10, border: '1px solid #d8dee4', background: '#f6f8fa' }}
                        >
                          <div>Repository id: {syncJob.repository_id}</div>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                            <span>Status</span>
                            <RepositorySyncJobStatusBadge status={syncJob.status} />
                          </div>
                          <div>Queued at: {syncJob.queued_at}</div>
                          <div>Started at: {syncJob.started_at ?? 'Not started'}</div>
                          <div>Finished at: {syncJob.finished_at ?? 'Not finished'}</div>
                          <a
                            href={`#/repos/${encodeURIComponent(syncJob.repository_id)}`}
                            aria-label={`Open repository detail for ${syncJob.repository_id}`}
                            style={{ color: '#0969da', fontWeight: 600 }}
                          >
                            Open repository detail
                          </a>
                          {syncJob.error ? <div aria-label={`Error details for ${syncJob.repository_id}`}>Error: {syncJob.error}</div> : null}
                        </div>
                      ))}
                    </div>
                  ) : null}
                </section>
              </article>
            );
          })}
        </div>
      ) : null}
    </Panel>
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

function diffTypeBadgeStyle(changeType: CommitDiffFile['change_type']): CSSProperties {
  const palette: Record<CommitDiffFile['change_type'], { background: string; color: string }> = {
    added: { background: '#dafbe1', color: '#1a7f37' },
    modified: { background: '#ddf4ff', color: '#0969da' },
    deleted: { background: '#ffebe9', color: '#cf222e' },
    renamed: { background: '#fff8c5', color: '#9a6700' },
  };

  return {
    ...searchMetaBadgeStyle,
    background: palette[changeType].background,
    color: palette[changeType].color,
    textTransform: 'uppercase',
  };
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

const diffFileCardStyle: CSSProperties = {
  padding: 12,
  border: '1px solid #d8dee4',
  borderRadius: 12,
  background: '#f6f8fa',
  display: 'grid',
  gap: 10,
};

const diffStatAdditionsStyle: CSSProperties = {
  ...searchMetaBadgeStyle,
  background: '#dafbe1',
  color: '#1a7f37',
};

const diffStatDeletionsStyle: CSSProperties = {
  ...searchMetaBadgeStyle,
  background: '#ffebe9',
  color: '#cf222e',
};

const diffPatchStyle: CSSProperties = {
  margin: 0,
  padding: 12,
  borderRadius: 10,
  background: '#0d1117',
  color: '#e6edf3',
  overflowX: 'auto',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  fontFamily: 'ui-monospace, SFMono-Regular, SFMono-Regular, Consolas, monospace',
  fontSize: 12,
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
    if (hash === '#/settings/connections') {
      return { kind: 'settings-connections' as const };
    }

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
        <nav style={{ display: 'flex', gap: 12, marginTop: 16 }}>
          <a href="#/" style={{ color: '#0969da', fontWeight: 600 }}>
            Repositories
          </a>
          <a href="#/settings/connections" style={{ color: '#0969da', fontWeight: 600 }}>
            Settings / Connections
          </a>
        </nav>
      </header>

      {route.kind === 'repo' ? <RepoDetailPage repoId={route.repoId} /> : null}
      {route.kind === 'settings-connections' ? <SettingsConnectionsPage /> : null}
      {route.kind === 'home' ? <RepoListPage /> : null}
    </main>
  );
}
