import { useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent, type MouseEvent, type ReactNode } from 'react';

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

type AskCitation = {
  repo_id: string;
  path: string;
  revision: string;
  line_start: number;
  line_end: number;
};

type AskRenderedCitation = AskCitation & {
  display_label: string;
  pinned_location: string;
  line_fragment: string;
};

type AskCompletionResponse = {
  provider: string;
  model: string | null;
  answer: string;
  citations: AskCitation[];
  rendered_citations: AskRenderedCitation[];
  thread_id: string;
  session_id: string;
};

type AskThreadSummary = {
  id: string;
  session_id: string;
  title: string;
  repo_scope: string[];
  visibility: string;
  updated_at: string;
  message_count: number;
};

type AskMessageResponse = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  citations: AskCitation[];
  rendered_citations: AskRenderedCitation[];
};

type AskThreadDetail = {
  id: string;
  session_id: string;
  user_id: string;
  title: string;
  repo_scope: string[];
  visibility: string;
  created_at: string;
  updated_at: string;
  messages: AskMessageResponse[];
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

type RepoRouteState = {
  repoId: string;
  initialPath: string | null;
  initialTreePath: string | null;
  initialRevision: string | null;
  from: 'search' | null;
  searchHash: string | null;
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

type ApiKeyListItem = {
  id: string;
  user_id: string;
  name: string;
  created_at: string;
  revoked_at: string | null;
  repo_scope: string[];
};

type CreateApiKeyResponse = ApiKeyListItem & {
  secret: string;
};

type OAuthClientListItem = {
  id: string;
  organization_id: string;
  name: string;
  client_id: string;
  redirect_uris: string[];
  created_by_user_id: string;
  created_at: string;
  revoked_at: string | null;
};

type CreateOAuthClientResponse = OAuthClientListItem & {
  client_secret: string;
};

type MembersAccount = {
  id: string;
  email: string;
  name: string;
  created_at?: string;
};

type MembersRosterEntry = {
  user_id: string;
  role: 'admin' | 'viewer';
  joined_at: string;
  account: MembersAccount;
};

type InviteRosterEntry = {
  id: string;
  email: string;
  role: 'admin' | 'viewer';
  created_at: string;
  expires_at: string;
  invited_by: MembersAccount | null;
  accepted_by: MembersAccount | null;
  accepted_at: string | null;
  status: 'accepted' | 'pending';
};

type MembersOrganizationInventory = {
  organization: {
    id: string;
    slug: string;
    name: string;
  };
  members: MembersRosterEntry[];
  invites: InviteRosterEntry[];
};

type LinkedAccountIdentity = {
  provider: string;
  user_id: string;
  email: string;
  name: string;
  created_at: string;
  primary: boolean;
};

type LinkedAccountMembership = {
  organization: {
    id: string;
    slug: string;
    name: string;
  };
  role: 'admin' | 'viewer';
  joined_at: string;
};

type LinkedAccountsInventory = {
  identities: LinkedAccountIdentity[];
  memberships: LinkedAccountMembership[];
  external_linking_supported: boolean;
};

type ReviewWebhookListItem = {
  id: string;
  organization_id: string;
  connection_id: string;
  repository_id: string;
  events: string[];
  created_by_user_id: string;
  created_at: string;
};

type ReviewWebhookDeliveryAttemptListItem = {
  id: string;
  webhook_id: string;
  connection_id: string;
  repository_id: string;
  event_type: string;
  review_id: string;
  external_event_id: string;
  accepted_at: string;
};

type ReviewAgentRunStatus = 'queued' | 'claimed' | 'completed' | 'failed';

type ReviewAgentRunListItem = {
  id: string;
  organization_id: string;
  webhook_id: string;
  delivery_attempt_id: string;
  connection_id: string;
  repository_id: string;
  review_id: string;
  status: ReviewAgentRunStatus;
  created_at: string;
};

type ReviewWebhookDetail = ReviewWebhookListItem;
type ReviewWebhookDeliveryAttemptDetail = ReviewWebhookDeliveryAttemptListItem;
type ReviewAgentRunDetail = ReviewAgentRunListItem;

type AuditActor = {
  user_id?: string;
  api_key_id?: string;
};

type AuditEventListItem = {
  id: string;
  organization_id: string;
  actor: AuditActor;
  action: string;
  target_type: string;
  target_id: string;
  occurred_at: string;
  metadata: unknown;
};

type AnalyticsRecordListItem = {
  id: string;
  organization_id: string;
  metric: string;
  recorded_at: string;
  value: unknown;
  dimensions: unknown;
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
  synced_revision?: string | null;
  synced_branch?: string | null;
  synced_content_file_count?: number | null;
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

type BootstrapStatusResponse = {
  bootstrap_required: boolean;
};

type LocalSessionToken = {
  sessionId: string;
  sessionSecret: string;
};

type LocalLoginResponse = {
  session_id: string;
  session_secret: string;
  user_id: string;
  created_at: string;
};

type AuthMeResponse = {
  user_id: string;
  email: string;
  name: string;
  session_id: string;
  created_at: string;
};

type OAuthCallbackContext = {
  provider: string | null;
  error: string | null;
  errorDescription: string | null;
  code: string | null;
  state: string | null;
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

const localSessionStorageKey = 'sourcebot-local-session';

function readStoredLocalSession(): LocalSessionToken | null {
  const rawValue = window.localStorage.getItem(localSessionStorageKey);
  if (!rawValue) {
    return null;
  }

  try {
    const parsed = JSON.parse(rawValue) as Partial<LocalSessionToken>;
    if (typeof parsed.sessionId === 'string' && typeof parsed.sessionSecret === 'string') {
      return {
        sessionId: parsed.sessionId,
        sessionSecret: parsed.sessionSecret,
      };
    }
  } catch {
    // Ignore malformed persisted state and treat it as absent.
  }

  window.localStorage.removeItem(localSessionStorageKey);
  return null;
}

function storeLocalSession(session: LocalSessionToken) {
  window.localStorage.setItem(localSessionStorageKey, JSON.stringify(session));
}

function clearStoredLocalSession() {
  window.localStorage.removeItem(localSessionStorageKey);
}

function mergeRequestHeaders(headers?: HeadersInit, additionalHeaders?: Record<string, string>): HeadersInit | undefined {
  if (!additionalHeaders || Object.keys(additionalHeaders).length === 0) {
    return headers;
  }

  const mergedEntries = new Map<string, string>();

  if (headers instanceof Headers) {
    headers.forEach((value, key) => {
      mergedEntries.set(key, value);
    });
  } else if (Array.isArray(headers)) {
    headers.forEach(([key, value]) => {
      mergedEntries.set(key, value);
    });
  } else if (headers) {
    Object.entries(headers).forEach(([key, value]) => {
      if (value !== undefined) {
        mergedEntries.set(key, String(value));
      }
    });
  }

  Object.entries(additionalHeaders).forEach(([key, value]) => {
    mergedEntries.set(key, value);
  });

  return Object.fromEntries(mergedEntries.entries());
}

function buildAuthenticatedRequestInit(path: string, init?: RequestInit): RequestInit | undefined {
  const needsAuthHeader =
    path.startsWith('/api/v1/') &&
    !path.startsWith('/api/v1/config') &&
    !path.startsWith('/api/v1/auth/bootstrap') &&
    !path.startsWith('/api/v1/auth/login') &&
    !path.startsWith('/api/v1/auth/invite-redeem');
  if (!needsAuthHeader) {
    return init;
  }

  const session = readStoredLocalSession();
  if (!session) {
    return init;
  }

  return {
    ...init,
    headers: mergeRequestHeaders(init?.headers, {
      Authorization: `Bearer ${session.sessionId}:${session.sessionSecret}`,
    }),
  };
}

async function authFetch(path: string, init?: RequestInit): Promise<Response> {
  const requestInit = buildAuthenticatedRequestInit(path, init);
  return requestInit ? fetch(path, requestInit) : fetch(path);
}

class HttpError extends Error {
  status: number;

  constructor(status: number) {
    super(`Request failed: ${status}`);
    this.name = 'HttpError';
    this.status = status;
  }
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await authFetch(path, init);
  if (!response.ok) {
    throw new HttpError(response.status);
  }
  return (await response.json()) as T;
}

function humanizeOAuthProvider(provider: string | null): string {
  if (!provider) {
    return 'External provider';
  }

  const knownProviders: Record<string, string> = {
    github: 'GitHub',
    gitlab: 'GitLab',
    gitea: 'Gitea',
    gerrit: 'Gerrit',
    bitbucket: 'Bitbucket',
    azure_devops: 'Azure DevOps',
  };

  if (knownProviders[provider]) {
    return knownProviders[provider];
  }

  return provider
    .split(/[_\s-]+/)
    .filter((segment) => segment.length > 0)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ');
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

function formatJsonValue(value: unknown): string {
  if (value === null || value === undefined) {
    return 'None';
  }

  if (typeof value === 'string') {
    return value;
  }

  try {
    const serialized = JSON.stringify(value);
    return serialized === undefined ? 'None' : serialized;
  } catch {
    return 'Unserializable value';
  }
}

function useRepoSummaries() {
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

  return { repos, loading, error };
}

function buildSearchHash(query: string, repoId: string) {
  const params = new URLSearchParams({ q: query, repo_id: repoId });
  return `#/search?${params.toString()}`;
}

function buildAskHash(repoId: string | null, threadId: string | null = null) {
  const params = new URLSearchParams();

  if (repoId && repoId.length > 0) {
    params.set('repo_id', repoId);
  }

  if (threadId && threadId.length > 0) {
    params.set('thread_id', threadId);
  }

  const query = params.toString();
  return `#/ask${query ? `?${query}` : ''}`;
}

function buildChatHash(repoId: string | null, threadId: string | null = null) {
  const params = new URLSearchParams();

  if (repoId && repoId.length > 0) {
    params.set('repo_id', repoId);
  }

  if (threadId && threadId.length > 0) {
    params.set('thread_id', threadId);
  }

  const query = params.toString();
  return `#/chat${query ? `?${query}` : ''}`;
}

function buildAgentsHash(runId: string | null = null) {
  const params = new URLSearchParams();

  if (runId && runId.length > 0) {
    params.set('run_id', runId);
  }

  const query = params.toString();
  return `#/agents${query ? `?${query}` : ''}`;
}

function reviewAgentRunStatusColor(status: ReviewAgentRunStatus) {
  switch (status) {
    case 'failed':
      return '#cf222e';
    case 'completed':
      return '#1a7f37';
    case 'claimed':
      return '#9a6700';
    case 'queued':
      return '#0969da';
  }
}

function ReviewAgentRunStatusBadge({ status }: { status: ReviewAgentRunStatus }) {
  return <span style={sharedStatusBadgeStyle(reviewAgentRunStatusColor(status))}>{status}</span>;
}

function buildRepoHash(
  repoId: string,
  options: {
    path: string | null;
    treePath: string | null;
    revision: string | null;
    from: 'search' | null;
    searchHash: string | null;
  }
) {
  const params = new URLSearchParams();
  const normalizedTreePath = options.treePath && options.treePath.length > 0 ? options.treePath : null;
  const canonicalTreePath = options.path ? pathDirectory(options.path) : null;

  if (options.path) {
    params.set('path', options.path);
  }

  if (normalizedTreePath && normalizedTreePath !== canonicalTreePath) {
    params.set('tree_path', normalizedTreePath);
  }

  if (options.from === 'search') {
    params.set('from', 'search');
    const [, searchQuery = ''] = (options.searchHash ?? '#/search').split('?');
    const searchParams = new URLSearchParams(searchQuery);
    params.set('q', searchParams.get('q') ?? '');
    params.set('repo_id', searchParams.get('repo_id') ?? '');
  }

  if (options.revision) {
    params.set('revision', options.revision);
  }

  const query = params.toString();
  return `#/repos/${encodeURIComponent(repoId)}${query ? `?${query}` : ''}`;
}

function navigateToHash(event: MouseEvent<HTMLAnchorElement>, targetHash: string) {
  event.preventDefault();
  window.location.hash = targetHash;
}

function SearchExperience({ repos, subtitle, initialQuery, initialRepoId }: { repos: RepoSummary[]; subtitle: string; initialQuery: string; initialRepoId: string }) {
  const [query, setQuery] = useState(initialQuery);
  const [selectedRepoId, setSelectedRepoId] = useState(initialRepoId);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [submittedQuery, setSubmittedQuery] = useState<string | null>(initialQuery.trim().length > 0 ? initialQuery.trim() : null);
  const [submittedRepoId, setSubmittedRepoId] = useState(initialRepoId);

  const repoNamesById = useMemo(() => new Map(repos.map((repo) => [repo.id, repo.name])), [repos]);

  const runSearch = async (queryValue = query, repoIdValue = selectedRepoId) => {
    const trimmedQuery = queryValue.trim();
    if (!trimmedQuery) {
      setSubmittedQuery(null);
      setSubmittedRepoId('');
      setSearchResults([]);
      setSearchError(null);
      return;
    }

    setSearchLoading(true);

    try {
      const params = new URLSearchParams({ q: trimmedQuery, repo_id: repoIdValue });
      const data = await fetchJson<SearchResponse>(`/api/v1/search?${params.toString()}`);
      setSearchResults(data.results);
      setSubmittedQuery(data.query);
      setSubmittedRepoId(data.repo_id ?? '');
      setSearchError(null);
    } catch (err) {
      setSearchResults([]);
      setSubmittedQuery(trimmedQuery);
      setSubmittedRepoId(repoIdValue);
      setSearchError((err as Error).message);
    } finally {
      setSearchLoading(false);
    }
  };

  useEffect(() => {
    if (initialQuery.trim().length === 0) {
      return;
    }

    void runSearch(initialQuery, initialRepoId);
  }, [initialQuery, initialRepoId]);

  return (
    <Panel title="Search" subtitle={subtitle}>
      <form
        style={{ display: 'grid', gap: 12 }}
        onSubmit={(event) => {
          event.preventDefault();
          const trimmedQuery = query.trim();

          if (trimmedQuery.length === 0) {
            window.location.hash = '#/search';
            return;
          }

          const targetHash = buildSearchHash(trimmedQuery, selectedRepoId);
          if (window.location.hash !== targetHash) {
            window.location.hash = targetHash;
            return;
          }

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
            <select value={selectedRepoId} onChange={(event) => setSelectedRepoId(event.target.value)} style={inputStyle}>
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
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, marginTop: 12 }}>
                  <a
                    href={`#/repos/${encodeURIComponent(result.repo_id)}?path=${encodeURIComponent(result.path)}&from=search&q=${encodeURIComponent(submittedQuery ?? '')}&repo_id=${encodeURIComponent(submittedRepoId)}`}
                    onClick={(event) =>
                      navigateToHash(
                        event,
                        `#/repos/${encodeURIComponent(result.repo_id)}?path=${encodeURIComponent(result.path)}&from=search&q=${encodeURIComponent(submittedQuery ?? '')}&repo_id=${encodeURIComponent(submittedRepoId)}`,
                      )
                    }
                    style={{ color: '#0969da', fontWeight: 600 }}
                  >
                    Open source in repository detail
                  </a>
                  <a
                    href={`#/repos/${encodeURIComponent(result.repo_id)}?from=search&q=${encodeURIComponent(submittedQuery ?? '')}&repo_id=${encodeURIComponent(submittedRepoId)}`}
                    onClick={(event) =>
                      navigateToHash(
                        event,
                        `#/repos/${encodeURIComponent(result.repo_id)}?from=search&q=${encodeURIComponent(submittedQuery ?? '')}&repo_id=${encodeURIComponent(submittedRepoId)}`,
                      )
                    }
                    style={{ color: '#0969da', fontWeight: 600 }}
                  >
                    Open repository detail
                  </a>
                </div>
              </div>
            ))}
          </div>
        ) : null}
        {!searchLoading && !searchError && !submittedQuery ? (
          <div style={{ color: '#57606a' }}>Enter a query to search indexed code.</div>
        ) : null}
      </div>
    </Panel>
  );
}

function SearchPage({ initialQuery, initialRepoId }: { initialQuery: string; initialRepoId: string }) {
  const { repos, loading, error } = useRepoSummaries();

  if (loading) return <Panel title="Search" subtitle="Run API-backed code search across repositories from a dedicated route.">Loading repositories…</Panel>;
  if (error) return <Panel title="Search" subtitle="Run API-backed code search across repositories from a dedicated route.">Failed to load: {error}</Panel>;

  return (
    <SearchExperience
      repos={repos}
      subtitle="Run API-backed code search across repositories from a dedicated route."
      initialQuery={initialQuery}
      initialRepoId={initialRepoId}
    />
  );
}

function ChatPage({ initialRepoId, initialThreadId }: { initialRepoId: string; initialThreadId: string | null }) {
  const { repos, loading, error } = useRepoSummaries();
  const [threadSummaries, setThreadSummaries] = useState<AskThreadSummary[]>([]);
  const [threadsLoading, setThreadsLoading] = useState(true);
  const [threadsError, setThreadsError] = useState<string | null>(null);
  const [selectedRepoId, setSelectedRepoId] = useState(initialRepoId);
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(initialThreadId);
  const [threadDetail, setThreadDetail] = useState<AskThreadDetail | null>(null);
  const [threadDetailLoading, setThreadDetailLoading] = useState(false);
  const [threadDetailError, setThreadDetailError] = useState<string | null>(null);
  const [prompt, setPrompt] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [threadTitleInput, setThreadTitleInput] = useState('');
  const [threadVisibilityInput, setThreadVisibilityInput] = useState('private');
  const [metadataSubmitting, setMetadataSubmitting] = useState(false);
  const [metadataMessage, setMetadataMessage] = useState<string | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const [deleteSubmitting, setDeleteSubmitting] = useState(false);
  const [deleteMessage, setDeleteMessage] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const selectedThreadIdRef = useRef<string | null>(initialThreadId);
  const metadataRequestVersionRef = useRef(0);
  const chatRequestVersionRef = useRef(0);
  const restoredChatThreadUnavailableMessage = 'The restored chat thread is no longer available for this repository scope. Start a fresh chat.';

  const invalidateInFlightChatRequest = () => {
    chatRequestVersionRef.current += 1;
    setSubmitting(false);
  };

  useEffect(() => {
    let cancelled = false;

    fetchJson<AskThreadSummary[]>('/api/v1/ask/threads')
      .then((data) => {
        if (!cancelled) {
          setThreadSummaries(data.filter((summary) => summary.repo_scope.length === 1));
          setThreadsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setThreadsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setThreadsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (repos.length === 0) {
      setSelectedRepoId('');
      return;
    }

    const requestedRepoId = initialRepoId.trim();
    const requestedRepoIsVisible = requestedRepoId.length > 0 && repos.some((repo) => repo.id === requestedRepoId);
    const nextRepoId = requestedRepoIsVisible
      ? requestedRepoId
      : selectedRepoId && repos.some((repo) => repo.id === selectedRepoId)
        ? selectedRepoId
        : repos[0].id;

    if (selectedThreadId) {
      return;
    }

    setSelectedRepoId(nextRepoId);

    const targetHash = buildChatHash(nextRepoId, null);
    if (!initialThreadId && window.location.hash !== targetHash) {
      window.location.hash = targetHash;
    }
  }, [initialRepoId, initialThreadId, repos, selectedRepoId, selectedThreadId]);

  useEffect(() => {
    selectedThreadIdRef.current = selectedThreadId;
  }, [selectedThreadId]);

  useEffect(() => {
    setSelectedThreadId((currentThreadId) => {
      if (currentThreadId === initialThreadId) {
        return currentThreadId;
      }
      invalidateInFlightChatRequest();
      metadataRequestVersionRef.current += 1;
      selectedThreadIdRef.current = initialThreadId;
      setMetadataSubmitting(false);
      setDeleteSubmitting(false);
      setThreadDetail(null);
      setThreadDetailError(null);
      setSubmitError(null);
      return initialThreadId;
    });
  }, [initialThreadId]);

  useEffect(() => {
    if (!selectedThreadId || repos.length === 0) {
      return;
    }

    let cancelled = false;
    setThreadDetailLoading(true);
    setThreadDetailError(null);

    fetchJson<AskThreadDetail>(`/api/v1/ask/threads/${encodeURIComponent(selectedThreadId)}`)
      .then((detail) => {
        if (cancelled) {
          return;
        }

        const nextRepoId = detail.repo_scope[0] ?? '';
        const supportsRepoScopedChat = detail.repo_scope.length === 1 && repos.some((repo) => repo.id === nextRepoId);
        if (!supportsRepoScopedChat) {
          throw new Error(restoredChatThreadUnavailableMessage);
        }

        setThreadDetail(detail);
        setThreadTitleInput(detail.title);
        setThreadVisibilityInput(detail.visibility);
        setMetadataMessage(null);
        setMetadataError(null);
        setSelectedRepoId(nextRepoId);
        setSubmitError(null);
        const targetHash = buildChatHash(nextRepoId, detail.id);
        if (window.location.hash !== targetHash) {
          window.location.hash = targetHash;
        }
      })
      .catch((err: Error) => {
        if (cancelled) {
          return;
        }

        setThreadDetail(null);
        setThreadDetailError(err.message);
        setSelectedThreadId(null);
        setSubmitError(null);
        const fallbackRepoId = selectedRepoId || initialRepoId || repos[0]?.id || null;
        if (fallbackRepoId) {
          setSelectedRepoId(fallbackRepoId);
        }
        const targetHash = buildChatHash(fallbackRepoId, null);
        if (window.location.hash !== targetHash) {
          window.location.hash = targetHash;
        }
      })
      .finally(() => {
        if (!cancelled) {
          setThreadDetailLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [initialRepoId, repos, restoredChatThreadUnavailableMessage, selectedRepoId, selectedThreadId]);

  const repoNamesById = useMemo(() => new Map(repos.map((repo) => [repo.id, repo.name])), [repos]);
  const selectedRepoName = selectedRepoId ? (repoNamesById.get(selectedRepoId) ?? selectedRepoId) : 'No repository selected';

  const handleStartFreshChat = () => {
    invalidateInFlightChatRequest();
    metadataRequestVersionRef.current += 1;
    selectedThreadIdRef.current = null;
    setMetadataSubmitting(false);
    setDeleteSubmitting(false);
    setSelectedThreadId(null);
    setThreadDetail(null);
    setThreadDetailError(null);
    setMetadataMessage(null);
    setMetadataError(null);
    setSubmitError(null);
    const fallbackRepoId = selectedRepoId || repos[0]?.id || null;
    if (fallbackRepoId) {
      setSelectedRepoId(fallbackRepoId);
    }
    const targetHash = buildChatHash(fallbackRepoId, null);
    if (window.location.hash !== targetHash) {
      window.location.hash = targetHash;
    }
  };

  const handleUpdateThreadMetadata = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!threadDetail || metadataSubmitting) {
      return;
    }

    const nextTitle = threadTitleInput.trim();
    if (nextTitle.length === 0) {
      setMetadataMessage(null);
      setMetadataError('Thread title cannot be empty.');
      return;
    }

    const submittedThreadId = threadDetail.id;
    const requestVersion = metadataRequestVersionRef.current + 1;
    metadataRequestVersionRef.current = requestVersion;
    setMetadataSubmitting(true);
    setMetadataMessage(null);
    setMetadataError(null);

    try {
      const updatedThread = await fetchJson<AskThreadDetail>(`/api/v1/ask/threads/${encodeURIComponent(submittedThreadId)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: nextTitle,
          visibility: threadVisibilityInput,
        }),
      });

      setThreadSummaries((current) =>
        current.map((summary) =>
          summary.id === updatedThread.id
            ? {
                ...summary,
                title: updatedThread.title,
                visibility: updatedThread.visibility,
                updated_at: updatedThread.updated_at,
                message_count: updatedThread.messages.length,
              }
            : summary
        )
      );

      const isCurrentMetadataRequest =
        metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId;
      if (!isCurrentMetadataRequest) {
        return;
      }

      setThreadDetail(updatedThread);
      setThreadTitleInput(updatedThread.title);
      setThreadVisibilityInput(updatedThread.visibility);
      setMetadataMessage('Thread metadata saved.');
    } catch (metadataUpdateError) {
      const isCurrentMetadataRequest =
        metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId;
      if (!isCurrentMetadataRequest) {
        return;
      }
      setMetadataError(metadataUpdateError instanceof Error ? metadataUpdateError.message : 'Unknown thread metadata error');
    } finally {
      if (metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId) {
        setMetadataSubmitting(false);
      }
    }
  };

  const handleDeleteThread = async () => {
    if (!threadDetail || deleteSubmitting) {
      return;
    }

    const submittedThreadId = threadDetail.id;
    const submittedRepoId = threadDetail.repo_scope[0] ?? selectedRepoId;
    const requestVersion = metadataRequestVersionRef.current + 1;
    metadataRequestVersionRef.current = requestVersion;
    setDeleteSubmitting(true);
    setDeleteMessage(null);
    setDeleteError(null);
    setMetadataMessage(null);
    setMetadataError(null);

    try {
      const response = await authFetch(`/api/v1/ask/threads/${encodeURIComponent(submittedThreadId)}`, { method: 'DELETE' });
      if (!response.ok) {
        throw new HttpError(response.status);
      }

      setThreadSummaries((current) => current.filter((summary) => summary.id !== submittedThreadId));

      const isCurrentDeleteRequest = metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId;
      if (!isCurrentDeleteRequest) {
        return;
      }

      invalidateInFlightChatRequest();
      selectedThreadIdRef.current = null;
      setSelectedThreadId(null);
      setThreadDetail(null);
      setThreadDetailError(null);
      setSubmitError(null);
      setDeleteMessage('Thread deleted.');
      const fallbackRepoId = submittedRepoId || selectedRepoId || repos[0]?.id || null;
      if (fallbackRepoId) {
        setSelectedRepoId(fallbackRepoId);
      }
      const targetHash = buildChatHash(fallbackRepoId, null);
      if (window.location.hash !== targetHash) {
        window.location.hash = targetHash;
      }
    } catch (threadDeleteError) {
      const isCurrentDeleteRequest = metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId;
      if (!isCurrentDeleteRequest) {
        return;
      }
      setDeleteError(threadDeleteError instanceof Error ? threadDeleteError.message : 'Unknown thread delete error');
    } finally {
      if (metadataRequestVersionRef.current === requestVersion && selectedThreadIdRef.current === submittedThreadId) {
        setDeleteSubmitting(false);
      }
    }
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (prompt.trim().length === 0 || !selectedRepoId) {
      return;
    }

    const requestPrompt = prompt.trim();
    const requestRepoScope = threadDetail?.repo_scope.length ? threadDetail.repo_scope : [selectedRepoId];
    if (requestRepoScope.length !== 1) {
      setSubmitError(restoredChatThreadUnavailableMessage);
      return;
    }

    const requestThreadId = selectedThreadId;
    const requestVersion = chatRequestVersionRef.current + 1;
    chatRequestVersionRef.current = requestVersion;

    setSubmitting(true);
    setSubmitError(null);

    try {
      const requestBody: { prompt: string; repo_scope: string[]; thread_id?: string } = {
        prompt: requestPrompt,
        repo_scope: requestRepoScope,
      };

      if (requestThreadId) {
        requestBody.thread_id = requestThreadId;
      }

      const response = await fetchJson<AskCompletionResponse>('/api/v1/ask/completions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(requestBody),
      });

      if (chatRequestVersionRef.current !== requestVersion) {
        return;
      }

      const nextThreadId = response.thread_id;
      const nextRepoId = requestRepoScope[0] ?? selectedRepoId;
      const nextMessages = [
        ...(threadDetail?.messages ?? []),
        {
          id: `${nextThreadId}-user-${(threadDetail?.messages.length ?? 0) + 1}`,
          role: 'user' as const,
          content: requestPrompt,
          citations: [],
          rendered_citations: [],
        },
        {
          id: `${nextThreadId}-assistant-${(threadDetail?.messages.length ?? 0) + 2}`,
          role: 'assistant' as const,
          content: response.answer,
          citations: response.citations,
          rendered_citations: response.rendered_citations,
        },
      ];

      setThreadDetail({
        id: nextThreadId,
        session_id: response.session_id,
        user_id: threadDetail?.user_id ?? '',
        title: threadDetail?.title ?? requestPrompt,
        repo_scope: requestRepoScope,
        visibility: threadDetail?.visibility ?? 'private',
        created_at: threadDetail?.created_at ?? '',
        updated_at: threadDetail?.updated_at ?? '',
        messages: nextMessages,
      });
      setSelectedThreadId(nextThreadId);
      setSelectedRepoId(nextRepoId);
      setThreadSummaries((current) => {
        const existingSummary = current.find((summary) => summary.id === nextThreadId);
        const nextSummary: AskThreadSummary = {
          id: nextThreadId,
          session_id: response.session_id,
          title: existingSummary?.title ?? threadDetail?.title ?? requestPrompt,
          repo_scope: requestRepoScope,
          visibility: existingSummary?.visibility ?? threadDetail?.visibility ?? 'private',
          updated_at: existingSummary?.updated_at ?? threadDetail?.updated_at ?? '',
          message_count: nextMessages.length,
        };
        return [nextSummary, ...current.filter((summary) => summary.id !== nextThreadId)];
      });
      setPrompt('');

      const targetHash = buildChatHash(nextRepoId, nextThreadId);
      if (window.location.hash !== targetHash) {
        window.location.hash = targetHash;
      }
    } catch (chatError) {
      if (chatRequestVersionRef.current !== requestVersion) {
        return;
      }

      const staleRestoredThread = chatError instanceof HttpError && chatError.status === 404 && requestThreadId;
      if (staleRestoredThread) {
        setSelectedThreadId(null);
        setThreadDetail(null);
        setThreadDetailError(null);
        const fallbackRepoId = requestRepoScope[0] ?? selectedRepoId;
        if (fallbackRepoId) {
          setSelectedRepoId(fallbackRepoId);
        }
        const targetHash = buildChatHash(fallbackRepoId, null);
        if (window.location.hash !== targetHash) {
          window.location.hash = targetHash;
        }
        setSubmitError(restoredChatThreadUnavailableMessage);
      } else {
        setSubmitError(chatError instanceof Error ? chatError.message : 'Unknown chat error');
      }
    } finally {
      if (chatRequestVersionRef.current === requestVersion) {
        setSubmitting(false);
      }
    }
  };

  if (loading) return <Panel title="Chat" subtitle="Browse and continue grounded ask threads from a dedicated chat route.">Loading visible repositories…</Panel>;
  if (error) return <Panel title="Chat" subtitle="Browse and continue grounded ask threads from a dedicated chat route.">Failed to load: {error}</Panel>;

  return (
    <Panel title="Chat" subtitle="Browse and continue grounded ask threads from a dedicated chat route.">
      {repos.length === 0 ? (
        <div style={{ color: '#57606a' }}>No visible repositories found.</div>
      ) : (
        <div style={{ display: 'grid', gap: 20 }}>
          <div style={{ display: 'grid', gap: 12 }}>
            <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
              <button type="button" style={primaryButtonStyle} onClick={handleStartFreshChat}>
                Start fresh chat
              </button>
              <div style={{ color: '#57606a' }}>Current scope: {selectedRepoName}</div>
              {selectedThreadId ? <div style={{ color: '#57606a' }}>Active thread: {selectedThreadId}</div> : null}
            </div>
            {threadsLoading ? <div style={{ color: '#57606a' }}>Loading threads…</div> : null}
            {threadsError ? <div style={{ color: '#cf222e' }}>Failed to load threads: {threadsError}</div> : null}
            {!threadsLoading && !threadsError ? (
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                {threadSummaries.map((summary) => (
                  <button
                    key={summary.id}
                    type="button"
                    onClick={() => {
                      invalidateInFlightChatRequest();
                      metadataRequestVersionRef.current += 1;
                      selectedThreadIdRef.current = summary.id;
                      setMetadataSubmitting(false);
                      setDeleteSubmitting(false);
                      const nextRepoId = summary.repo_scope[0] ?? selectedRepoId;
                      setSelectedRepoId(nextRepoId);
                      setSelectedThreadId(summary.id);
                      setThreadDetail(null);
                      setThreadDetailError(null);
                      setSubmitError(null);
                      const targetHash = buildChatHash(nextRepoId, summary.id);
                      if (window.location.hash !== targetHash) {
                        window.location.hash = targetHash;
                      }
                    }}
                    style={{
                      ...detailCardStyle,
                      cursor: 'pointer',
                      textAlign: 'left',
                      background: summary.id === selectedThreadId ? '#ddf4ff' : '#ffffff',
                    }}
                  >
                    <div style={{ fontWeight: 600 }}>{summary.title}</div>
                    <div style={{ color: '#57606a', marginTop: 4 }}>{summary.message_count} messages</div>
                  </button>
                ))}
              </div>
            ) : null}
          </div>

          <form style={{ display: 'grid', gap: 12 }} onSubmit={(event) => void handleSubmit(event)}>
            <label style={fieldLabelStyle}>
              <span>Question</span>
              <textarea
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                placeholder="Continue the selected thread or start a fresh repository-scoped chat"
                rows={4}
                style={{ ...inputStyle, resize: 'vertical' }}
              />
            </label>

            <div>
              <button type="submit" style={primaryButtonStyle} disabled={submitting || prompt.trim().length === 0 || !selectedRepoId}>
                {submitting ? 'Asking…' : 'Ask'}
              </button>
            </div>
          </form>

          {threadDetailLoading ? <div style={{ color: '#57606a' }}>Loading thread…</div> : null}
          {threadDetailError ? <div style={{ color: '#cf222e' }}>Failed to load thread: {threadDetailError}</div> : null}
          {submitError ? <div style={{ color: '#cf222e' }}>Chat failed: {submitError}</div> : null}
          {deleteMessage ? <div style={{ color: '#1a7f37' }}>{deleteMessage}</div> : null}
          {deleteError ? <div style={{ color: '#cf222e' }}>Thread delete failed: {deleteError}</div> : null}

          {threadDetail ? (
            <div style={{ display: 'grid', gap: 12 }}>
              <h3 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{threadDetail.title}</h3>
              <div style={{ color: '#57606a' }}>Visibility: {threadDetail.visibility}</div>
              <form style={{ ...detailCardStyle, display: 'grid', gap: 12 }} onSubmit={(event) => void handleUpdateThreadMetadata(event)}>
                <div style={{ fontWeight: 700 }}>Thread metadata</div>
                <div style={searchFormGridStyle}>
                  <label style={fieldLabelStyle}>
                    <span>Thread title</span>
                    <input value={threadTitleInput} onChange={(event) => setThreadTitleInput(event.target.value)} style={inputStyle} />
                  </label>
                  <label style={fieldLabelStyle}>
                    <span>Thread visibility</span>
                    <select value={threadVisibilityInput} onChange={(event) => setThreadVisibilityInput(event.target.value)} style={inputStyle}>
                      <option value="private">private</option>
                      <option value="shared">shared</option>
                    </select>
                  </label>
                </div>
                <div>
                  <button type="submit" style={primaryButtonStyle} disabled={metadataSubmitting || threadTitleInput.trim().length === 0}>
                    {metadataSubmitting ? 'Saving…' : 'Save thread metadata'}
                  </button>
                </div>
                {metadataMessage ? <div style={{ color: '#1a7f37' }}>{metadataMessage}</div> : null}
                {metadataError ? <div style={{ color: '#cf222e' }}>Thread metadata failed: {metadataError}</div> : null}
              </form>
              <div>
                <button type="button" style={secondaryButtonStyle} onClick={() => void handleDeleteThread()} disabled={deleteSubmitting}>
                  {deleteSubmitting ? 'Deleting…' : 'Delete thread'}
                </button>
              </div>
              {threadDetail.messages.map((message) => (
                <div key={message.id} style={{ ...detailCardStyle, display: 'grid', gap: 8 }}>
                  <div style={{ color: '#57606a', fontWeight: 600, textTransform: 'capitalize' }}>{message.role}</div>
                  <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{message.content}</div>
                </div>
              ))}
            </div>
          ) : !threadDetailLoading && !threadDetailError ? (
            <div style={{ color: '#57606a' }}>Choose a thread to continue it, or start a fresh chat in the current repository scope.</div>
          ) : null}
        </div>
      )}
    </Panel>
  );
}

function AskPage({ initialRepoId, initialThreadId }: { initialRepoId: string; initialThreadId: string | null }) {
  const { repos, loading, error } = useRepoSummaries();
  const [prompt, setPrompt] = useState('');
  const [selectedRepoId, setSelectedRepoId] = useState(initialRepoId);
  const [activeThreadId, setActiveThreadId] = useState<string | null>(initialThreadId);
  const [answer, setAnswer] = useState<AskCompletionResponse | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const askRequestVersionRef = useRef(0);

  const invalidateInFlightAskRequest = () => {
    askRequestVersionRef.current += 1;
    setSubmitting(false);
  };

  useEffect(() => {
    if (repos.length === 0) {
      setSelectedRepoId('');
      return;
    }

    const requestedRepoId = initialRepoId.trim();
    const requestedRepoIsVisible = requestedRepoId.length > 0 && repos.some((repo) => repo.id === requestedRepoId);
    const nextRepoId =
      requestedRepoIsVisible
        ? requestedRepoId
        : selectedRepoId && repos.some((repo) => repo.id === selectedRepoId)
          ? selectedRepoId
          : repos[0].id;

    setSelectedRepoId((currentRepoId) => {
      if (currentRepoId === nextRepoId) {
        return currentRepoId;
      }
      invalidateInFlightAskRequest();
      setAnswer(null);
      setSubmitError(null);
      return nextRepoId;
    });

    if (!requestedRepoIsVisible) {
      const targetHash = buildAskHash(nextRepoId, null);
      if (window.location.hash !== targetHash) {
        window.location.hash = targetHash;
      }
    }
  }, [initialRepoId, repos, selectedRepoId]);

  useEffect(() => {
    const requestedRepoId = initialRepoId.trim();
    const canRestoreThread =
      requestedRepoId.length > 0 && repos.some((repo) => repo.id === requestedRepoId);
    const requestedThreadId = canRestoreThread ? initialThreadId : null;

    setActiveThreadId((currentThreadId) => {
      if (currentThreadId === requestedThreadId) {
        return currentThreadId;
      }
      invalidateInFlightAskRequest();
      setAnswer(null);
      setSubmitError(null);
      return requestedThreadId;
    });
  }, [initialRepoId, initialThreadId, repos]);

  const selectedRepo = repos.find((repo) => repo.id === selectedRepoId) ?? null;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (prompt.trim().length === 0 || !selectedRepoId) {
      return;
    }

    const requestRepoId = selectedRepoId;
    const requestVersion = askRequestVersionRef.current + 1;
    askRequestVersionRef.current = requestVersion;

    setSubmitting(true);
    setSubmitError(null);

    try {
      const requestBody: { prompt: string; repo_scope: string[]; thread_id?: string } = {
        prompt: prompt.trim(),
        repo_scope: [requestRepoId],
      };

      if (activeThreadId) {
        requestBody.thread_id = activeThreadId;
      }

      const response = await fetchJson<AskCompletionResponse>('/api/v1/ask/completions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(requestBody),
      });

      if (askRequestVersionRef.current !== requestVersion) {
        return;
      }

      setAnswer(response);
      setActiveThreadId(response.thread_id);

      const targetHash = buildAskHash(requestRepoId, response.thread_id);
      if (window.location.hash !== targetHash) {
        window.location.hash = targetHash;
      }
    } catch (askError) {
      if (askRequestVersionRef.current !== requestVersion) {
        return;
      }
      const staleRestoredThread = askError instanceof HttpError && askError.status === 404 && activeThreadId;
      setAnswer(null);
      if (staleRestoredThread) {
        setActiveThreadId(null);
        const targetHash = buildAskHash(requestRepoId, null);
        if (window.location.hash !== targetHash) {
          window.location.hash = targetHash;
        }
        setSubmitError('The restored ask thread is no longer available for this repository scope. Start a fresh thread.');
      } else {
        setSubmitError(askError instanceof Error ? askError.message : 'Unknown ask error');
      }
    } finally {
      if (askRequestVersionRef.current === requestVersion) {
        setSubmitting(false);
      }
    }
  };

  if (loading) return <Panel title="Ask" subtitle="Ask grounded questions across visible repositories from a dedicated route.">Loading visible repositories…</Panel>;
  if (error) return <Panel title="Ask" subtitle="Ask grounded questions across visible repositories from a dedicated route.">Failed to load: {error}</Panel>;

  return (
    <Panel title="Ask" subtitle="Ask grounded questions across visible repositories from a dedicated route.">
      {repos.length === 0 ? (
        <div style={{ display: 'grid', gap: 8 }}>
          <div>No visible repositories found</div>
          <div style={{ color: '#57606a' }}>
            Your current account does not have any visible repositories yet. Full ask history management and agents parity remain follow-up work.
          </div>
        </div>
      ) : (
        <div style={{ display: 'grid', gap: 20 }}>
          <form style={{ display: 'grid', gap: 12 }} onSubmit={(event) => void handleSubmit(event)}>
            <div style={searchFormGridStyle}>
              <label style={fieldLabelStyle}>
                <span>Repository scope</span>
                <select
                  value={selectedRepoId}
                  onChange={(event) => {
                    const nextRepoId = event.target.value;
                    invalidateInFlightAskRequest();
                    setSelectedRepoId(nextRepoId);
                    setActiveThreadId(null);
                    setAnswer(null);
                    setSubmitError(null);

                    const targetHash = buildAskHash(nextRepoId, null);
                    if (window.location.hash !== targetHash) {
                      window.location.hash = targetHash;
                    }
                  }}
                  style={inputStyle}
                >
                  {repos.map((repo) => (
                    <option key={repo.id} value={repo.id}>
                      {repo.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <label style={fieldLabelStyle}>
              <span>Question</span>
              <textarea
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                placeholder="Ask a codebase question grounded in the selected repository"
                rows={4}
                style={{ ...inputStyle, resize: 'vertical' }}
              />
            </label>

            <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
              <button type="submit" style={primaryButtonStyle} disabled={submitting || prompt.trim().length === 0 || !selectedRepoId}>
                {submitting ? 'Asking…' : 'Ask'}
              </button>
              <div style={{ color: '#57606a' }}>Current scope: {selectedRepo?.name ?? selectedRepoId}</div>
              {activeThreadId ? <div style={{ color: '#57606a' }}>Active thread: {activeThreadId}</div> : null}
            </div>
          </form>

          {submitting ? <div style={{ color: '#57606a' }}>Loading answer…</div> : null}
          {submitError ? <div style={{ color: '#cf222e' }}>Ask failed: {submitError}</div> : null}
          {!submitting && !submitError && !answer ? (
            <div style={{ color: '#57606a' }}>Choose a repository scope and submit a question to the ask completions API.</div>
          ) : null}

          {answer ? (
            <div style={{ ...detailCardStyle, display: 'grid', gap: 12 }}>
              <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
                <span style={searchMetaBadgeStyle}>Provider: {answer.provider}</span>
                <span style={searchMetaBadgeStyle}>Model: {answer.model ?? 'unknown'}</span>
                <span style={searchMetaBadgeStyle}>Session: {answer.session_id}</span>
              </div>
              <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{answer.answer}</div>
              {answer.rendered_citations.length > 0 ? (
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                  <strong>Inline citations:</strong>
                  {answer.rendered_citations.map((citation) => (
                    <a
                      key={`${citation.repo_id}:${citation.path}:${citation.line_start}:${citation.line_end}`}
                      href={buildRepoHash(citation.repo_id, {
                        path: citation.path,
                        treePath: null,
                        revision: citation.revision,
                        from: null,
                        searchHash: null,
                      })}
                      style={{ color: '#0969da' }}
                    >
                      {citation.display_label}
                    </a>
                  ))}
                </div>
              ) : (
                <div style={{ color: '#57606a' }}>No citations were returned for this answer.</div>
              )}
            </div>
          ) : null}
        </div>
      )}
    </Panel>
  );
}

function RepoListPage() {
  const { repos, loading, error } = useRepoSummaries();

  if (loading) return <Panel title="Repositories">Loading repositories…</Panel>;
  if (error) return <Panel title="Repositories">Failed to load: {error}</Panel>;

  return (
    <div style={{ display: 'grid', gap: 20 }}>
      <Panel
        title="Search"
        subtitle="Use the dedicated search page for the current API-backed code-search flow while the repository home stays focused on inventory and navigation."
      >
        <a href="#/search" style={{ ...primaryButtonStyle, display: 'inline-flex', alignItems: 'center', textDecoration: 'none' }}>
          Open dedicated search page
        </a>
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

function AuthPage({
  inviteId = null,
  inviteEmail = null,
  oauthCallback = null,
}: {
  inviteId?: string | null;
  inviteEmail?: string | null;
  oauthCallback?: OAuthCallbackContext | null;
}) {
  const [bootstrapStatus, setBootstrapStatus] = useState<BootstrapStatusResponse | null>(null);
  const [identity, setIdentity] = useState<AuthMeResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [bootstrapSubmitting, setBootstrapSubmitting] = useState(false);
  const [loginSubmitting, setLoginSubmitting] = useState(false);
  const [inviteSubmitting, setInviteSubmitting] = useState(false);
  const [logoutSubmitting, setLogoutSubmitting] = useState(false);
  const [bootstrapName, setBootstrapName] = useState('');
  const [bootstrapEmail, setBootstrapEmail] = useState('');
  const [bootstrapPassword, setBootstrapPassword] = useState('');
  const [loginEmail, setLoginEmail] = useState('');
  const [loginPassword, setLoginPassword] = useState('');
  const [inviteName, setInviteName] = useState('');
  const [inviteEmailInput, setInviteEmailInput] = useState(inviteEmail ?? '');
  const [invitePassword, setInvitePassword] = useState('');
  const hasInviteContext = Boolean(inviteId && inviteEmail);
  const oauthProviderName = humanizeOAuthProvider(oauthCallback?.provider ?? null);
  const hasOAuthCallbackContext = Boolean(
    oauthCallback && (oauthCallback.provider || oauthCallback.error || oauthCallback.errorDescription || oauthCallback.code || oauthCallback.state)
  );
  const isOAuthErrorCallback = Boolean(oauthCallback?.error || oauthCallback?.errorDescription);
  const oauthCallbackNotice = hasOAuthCallbackContext ? (
    <div
      style={{
        ...detailCardStyle,
        borderColor: isOAuthErrorCallback ? '#cf222e' : '#0969da',
        background: isOAuthErrorCallback ? '#fff8f8' : '#f6f8ff',
        display: 'grid',
        gap: 8,
      }}
    >
      <div style={{ fontSize: 18, fontWeight: 700 }}>
        {isOAuthErrorCallback
          ? `OAuth callback parameters for ${oauthProviderName} indicate sign-in did not complete.`
          : `This auth route received OAuth callback parameters for ${oauthProviderName}.`}
      </div>
      <div style={{ color: '#57606a' }}>
        {isOAuthErrorCallback
          ? 'This rewrite does not finish external-provider sign-in on this screen yet.'
          : 'This rewrite does not finish external-provider sign-in here yet, so use local login below if you need access right now.'}
      </div>
      {oauthCallback?.provider ? <div>Provider: {oauthProviderName}</div> : null}
      {oauthCallback?.error ? <div>OAuth error: {oauthCallback.error}</div> : null}
      {oauthCallback?.errorDescription ? (
        <div>
          <div>Description</div>
          <div>{oauthCallback.errorDescription}</div>
        </div>
      ) : null}
      {oauthCallback?.code ? <div>An authorization code was received, but it is not exchanged on this screen yet.</div> : null}
      {oauthCallback?.state ? <div>An OAuth state parameter was also present.</div> : null}
    </div>
  ) : null;

  useEffect(() => {
    setInviteEmailInput(inviteEmail ?? '');
    setInviteName('');
    setInvitePassword('');
  }, [inviteEmail]);

  const loadBootstrapStatus = async () => {
    const status = await fetchJson<BootstrapStatusResponse>('/api/v1/auth/bootstrap');
    setBootstrapStatus(status);
    return status;
  };

  const restoreExistingSession = async () => {
    const session = readStoredLocalSession();
    if (!session) {
      return false;
    }

    try {
      const restoredIdentity = await fetchJson<AuthMeResponse>('/api/v1/auth/me');
      setIdentity(restoredIdentity);
      return true;
    } catch {
      clearStoredLocalSession();
      return false;
    }
  };

  const refreshAuthPage = async () => {
    setLoading(true);
    setError(null);

    try {
      const restored = await restoreExistingSession();
      if (!restored) {
        setIdentity(null);
        await loadBootstrapStatus();
      }
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : 'Unknown auth error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refreshAuthPage();
  }, []);

  const completeLocalSessionLogin = async (loginResponse: LocalLoginResponse) => {
    storeLocalSession({
      sessionId: loginResponse.session_id,
      sessionSecret: loginResponse.session_secret,
    });

    const restoredIdentity = await fetchJson<AuthMeResponse>('/api/v1/auth/me');
    setIdentity(restoredIdentity);
    setBootstrapStatus({ bootstrap_required: false });
  };

  const handleBootstrapSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setBootstrapSubmitting(true);
    setError(null);

    try {
      await fetchJson<BootstrapStatusResponse>('/api/v1/auth/bootstrap', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: bootstrapName,
          email: bootstrapEmail,
          password: bootstrapPassword,
        }),
      });

      setBootstrapStatus({ bootstrap_required: false });
      setLoginEmail(bootstrapEmail);
      setLoginPassword(bootstrapPassword);
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : 'Unknown auth error');
    } finally {
      setBootstrapSubmitting(false);
    }
  };

  const handleLoginSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLoginSubmitting(true);
    setError(null);

    try {
      const loginResponse = await fetchJson<LocalLoginResponse>('/api/v1/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: loginEmail,
          password: loginPassword,
        }),
      });

      await completeLocalSessionLogin(loginResponse);
    } catch (authError) {
      clearStoredLocalSession();
      setError(authError instanceof Error ? authError.message : 'Unknown auth error');
    } finally {
      setLoginSubmitting(false);
    }
  };

  const handleInviteRedeemSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!inviteId) {
      return;
    }

    setInviteSubmitting(true);
    setError(null);

    try {
      const loginResponse = await fetchJson<LocalLoginResponse>('/api/v1/auth/invite-redeem', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          invite_id: inviteId,
          email: inviteEmailInput,
          name: inviteName,
          password: invitePassword,
        }),
      });

      await completeLocalSessionLogin(loginResponse);
    } catch (authError) {
      clearStoredLocalSession();
      setError(authError instanceof Error ? authError.message : 'Unknown auth error');
    } finally {
      setInviteSubmitting(false);
    }
  };

  const handleLogout = async () => {
    setLogoutSubmitting(true);
    setError(null);

    try {
      const response = await authFetch('/api/v1/auth/logout', { method: 'POST' });
      if (!response.ok) {
        throw new Error(`Request failed: ${response.status}`);
      }
      clearStoredLocalSession();
      setIdentity(null);
      await refreshAuthPage();
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : 'Unknown auth error');
    } finally {
      setLogoutSubmitting(false);
    }
  };

  if (loading) {
    return <Panel title="Authentication" subtitle="First-run onboarding, local login, invite redemption, and session restoration.">Checking auth state…</Panel>;
  }

  if (identity) {
    return (
      <Panel title="Authentication" subtitle="First-run onboarding, local login, invite redemption, and session restoration.">
        <div style={{ display: 'grid', gap: 12 }}>
          {oauthCallbackNotice}
          <div style={{ fontSize: 18, fontWeight: 700 }}>Signed in as {identity.name}</div>
          <div style={{ color: '#57606a' }}>{identity.email}</div>
          <div style={{ color: '#57606a' }}>Session id: {identity.session_id}</div>
          <div>
            <button type="button" onClick={() => void handleLogout()} style={secondaryButtonStyle} disabled={logoutSubmitting}>
              {logoutSubmitting ? 'Logging out…' : 'Log out'}
            </button>
          </div>
          {error ? <div style={{ color: '#cf222e' }}>Authentication error: {error}</div> : null}
        </div>
      </Panel>
    );
  }

  if (bootstrapStatus?.bootstrap_required) {
    return (
      <Panel title="Authentication" subtitle="First-run onboarding, local login, invite redemption, and session restoration.">
        <div style={{ display: 'grid', gap: 16 }}>
          {oauthCallbackNotice}
          <div>
            <h2 style={{ margin: 0, fontSize: 24 }}>First-run onboarding</h2>
            <p style={{ color: '#57606a', marginTop: 8, marginBottom: 0 }}>
              Create the first local admin for this self-hosted rewrite instance.
            </p>
          </div>
          <form onSubmit={handleBootstrapSubmit} style={{ display: 'grid', gap: 12, maxWidth: 420 }}>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Name</span>
              <input value={bootstrapName} onChange={(event) => setBootstrapName(event.target.value)} required />
            </label>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Email address</span>
              <input type="email" value={bootstrapEmail} onChange={(event) => setBootstrapEmail(event.target.value)} required />
            </label>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Password</span>
              <input type="password" value={bootstrapPassword} onChange={(event) => setBootstrapPassword(event.target.value)} required />
            </label>
            <div>
              <button type="submit" style={primaryButtonStyle} disabled={bootstrapSubmitting}>
                {bootstrapSubmitting ? 'Creating first admin account…' : 'Create first admin account'}
              </button>
            </div>
          </form>
          {error ? <div style={{ color: '#cf222e' }}>Authentication error: {error}</div> : null}
        </div>
      </Panel>
    );
  }

  if (hasInviteContext) {
    return (
      <Panel title="Authentication" subtitle="First-run onboarding, local login, invite redemption, and session restoration.">
        <div style={{ display: 'grid', gap: 16 }}>
          {oauthCallbackNotice}
          <div>
            <h2 style={{ margin: 0, fontSize: 24 }}>Invite redemption</h2>
            <p style={{ color: '#57606a', marginTop: 8, marginBottom: 0 }}>
              Finish accepting your local workspace invite and create the password for this account.
            </p>
          </div>
          <form onSubmit={handleInviteRedeemSubmit} style={{ display: 'grid', gap: 12, maxWidth: 420 }}>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Name</span>
              <input value={inviteName} onChange={(event) => setInviteName(event.target.value)} required />
            </label>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Email address</span>
              <input type="email" value={inviteEmailInput} onChange={(event) => setInviteEmailInput(event.target.value)} required />
            </label>
            <label style={{ display: 'grid', gap: 6 }}>
              <span>Password</span>
              <input type="password" value={invitePassword} onChange={(event) => setInvitePassword(event.target.value)} required />
            </label>
            <div>
              <button type="submit" style={primaryButtonStyle} disabled={inviteSubmitting}>
                {inviteSubmitting ? 'Accepting invite and signing in…' : 'Accept invite and sign in'}
              </button>
            </div>
          </form>
          {error ? <div style={{ color: '#cf222e' }}>Authentication error: {error}</div> : null}
        </div>
      </Panel>
    );
  }

  return (
    <Panel title="Authentication" subtitle="First-run onboarding, local login, invite redemption, and session restoration.">
      <div style={{ display: 'grid', gap: 16 }}>
        {oauthCallbackNotice}
        <div>
          <h2 style={{ margin: 0, fontSize: 24 }}>Local login</h2>
          <p style={{ color: '#57606a', marginTop: 8, marginBottom: 0 }}>
            Sign in with a local account or use an invite redemption link to finish setting up an invited account.
          </p>
        </div>
        <form onSubmit={handleLoginSubmit} style={{ display: 'grid', gap: 12, maxWidth: 420 }}>
          <label style={{ display: 'grid', gap: 6 }}>
            <span>Email address</span>
            <input type="email" value={loginEmail} onChange={(event) => setLoginEmail(event.target.value)} required />
          </label>
          <label style={{ display: 'grid', gap: 6 }}>
            <span>Password</span>
            <input type="password" value={loginPassword} onChange={(event) => setLoginPassword(event.target.value)} required />
          </label>
          <div>
            <button type="submit" style={primaryButtonStyle} disabled={loginSubmitting}>
              {loginSubmitting ? 'Signing in locally…' : 'Sign in locally'}
            </button>
          </div>
        </form>
        {error ? <div style={{ color: '#cf222e' }}>Authentication error: {error}</div> : null}
      </div>
    </Panel>
  );
}

function RepoDetailPage({
  repoId,
  initialPath,
  initialTreePath,
  initialRevision,
  from,
  searchHash,
}: {
  repoId: string;
  initialPath: string | null;
  initialTreePath: string | null;
  initialRevision: string | null;
  from: 'search' | null;
  searchHash: string | null;
}) {
  const [repo, setRepo] = useState<RepoDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detailRequestKey, setDetailRequestKey] = useState(0);
  const [revisionInput, setRevisionInput] = useState(initialRevision ?? '');
  const backHref = from === 'search' ? searchHash ?? '#/search' : '#/';
  const backLabel = from === 'search' ? '← Back to search results' : '← Back to repositories';

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
  }, [repoId, detailRequestKey]);

  useEffect(() => {
    setRevisionInput(initialRevision ?? '');
  }, [initialRevision]);

  const updateRepoHash = (path: string | null, treePath: string | null, revision: string | null) => {
    const targetHash = buildRepoHash(repoId, { path, treePath, revision, from, searchHash });
    if (window.location.hash !== targetHash) {
      window.location.hash = targetHash;
    }
  };

  const backLink = (
    <div style={{ marginTop: 16 }}>
      <a href={backHref} onClick={(event) => navigateToHash(event, backHref)} style={{ color: '#0969da', textDecoration: 'none', fontWeight: 600 }}>
        {backLabel}
      </a>
    </div>
  );

  if (loading) {
    return (
      <Panel title="Repository detail" subtitle={`Repository id: ${repoId}`}>
        <div>Loading repository…</div>
        {backLink}
      </Panel>
    );
  }

  if (error) {
    return (
      <Panel title="Repository detail" subtitle={`Repository id: ${repoId}`}>
        <div style={{ display: 'grid', gap: 12 }}>
          <div>Failed to load repository: {error}</div>
          <div>
            <button type="button" style={secondaryButtonStyle} onClick={() => setDetailRequestKey((value) => value + 1)}>
              Retry loading repository
            </button>
          </div>
        </div>
        {backLink}
      </Panel>
    );
  }

  if (!repo) {
    return (
      <Panel title="Repository detail" subtitle={`Repository id: ${repoId}`}>
        <div>Repository not found.</div>
        {backLink}
      </Panel>
    );
  }

  return (
    <Panel title={repo.repository.name} subtitle={`Repository id: ${repo.repository.id}`}>
      <div style={detailGridStyle}>
        <Detail label="Default branch" value={repo.repository.default_branch} />
        <Detail label="Sync state" value={<StatusBadge state={repo.repository.sync_state} />} />
        <Detail label="Connection" value={repo.connection.name} />
        <Detail label="Connection kind" value={repo.connection.kind} />
      </div>
      <div style={{ marginTop: 20, padding: 16, borderRadius: 12, border: '1px solid #d8dee4', background: '#fff' }}>
        <div style={{ display: 'grid', gap: 12 }}>
          <label style={fieldLabelStyle}>
            <span>Branch, tag, or revision</span>
            <input value={revisionInput} onChange={(event) => setRevisionInput(event.target.value)} style={inputStyle} />
          </label>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            <button
              type="button"
              style={primaryButtonStyle}
              onClick={() => updateRepoHash(initialPath, initialTreePath, revisionInput.trim() || null)}
            >
              Apply revision
            </button>
            <button
              type="button"
              style={secondaryButtonStyle}
              onClick={() => updateRepoHash(initialPath, initialTreePath, repo.repository.default_branch || null)}
              disabled={!initialRevision && revisionInput.trim().length === 0}
            >
              Reset to default branch
            </button>
          </div>
        </div>
      </div>
      <div style={{ marginTop: 20 }}>
        <CommitsPanel repoId={repoId} revision={initialRevision} />
      </div>
      <div style={{ marginTop: 20 }}>
        <BrowsePanel
          key={repoId}
          repoId={repoId}
          initialPath={initialPath}
          initialTreePath={initialTreePath}
          initialRevision={initialRevision}
          onRouteChange={updateRepoHash}
        />
      </div>
      {backLink}
    </Panel>
  );
}

function CommitsPanel({ repoId, revision }: { repoId: string; revision: string | null }) {
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

    const params = new URLSearchParams({ limit: '20' });
    if (revision) {
      params.set('revision', revision);
    }

    fetchJson<CommitsResponse>(`/api/v1/repos/${repoId}/commits?${params.toString()}`)
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
  }, [repoId, revision]);

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

function BrowsePanel({
  repoId,
  initialPath,
  initialTreePath,
  initialRevision,
  onRouteChange,
}: {
  repoId: string;
  initialPath: string | null;
  initialTreePath: string | null;
  initialRevision: string | null;
  onRouteChange: (path: string | null, treePath: string | null, revision: string | null) => void;
}) {
  const [treePath, setTreePath] = useState(() => (initialTreePath ?? (initialPath ? pathDirectory(initialPath) : '')));
  const [tree, setTree] = useState<TreeResponse | null>(null);
  const [treeLoading, setTreeLoading] = useState(true);
  const [treeError, setTreeError] = useState<string | null>(null);
  const [treeRequestKey, setTreeRequestKey] = useState(0);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(initialPath);
  const [selectedRevision, setSelectedRevision] = useState<string | null>(initialRevision);
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

    const params = new URLSearchParams({ path: treePath });
    if (selectedRevision) {
      params.set('revision', selectedRevision);
    }

    fetchJson<TreeResponse>(`/api/v1/repos/${repoId}/tree?${params.toString()}`)
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
  }, [repoId, treePath, treeRequestKey, selectedRevision]);

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
    const nextTreePath = initialTreePath ?? (initialPath ? pathDirectory(initialPath) : '');
    const pathChanged = selectedFilePath !== initialPath;
    const revisionChanged = selectedRevision !== initialRevision;
    const treeChanged = treePath !== nextTreePath;

    if (!pathChanged && !revisionChanged && !treeChanged) {
      return;
    }

    navigationRequestId.current += 1;
    setTreePath(nextTreePath);
    if (pathChanged) {
      setSelectedFilePath(initialPath);
      setBlob(null);
      setBlobError(null);
      setBlobLoading(false);
      setSymbol('');
      setNavigationLoading(false);
      setNavigationError(null);
      setNavigationState(null);
    }
    if (revisionChanged) {
      setSelectedRevision(initialRevision);
      setSelectedFilePath(null);
      setBlob(null);
      setBlobError(null);
      setBlobLoading(false);
      setSymbol('');
      setNavigationLoading(false);
      setNavigationError(null);
      setNavigationState(null);
    }
  }, [repoId, initialPath, initialTreePath, initialRevision]);

  useEffect(() => {
    if (treeLoading || treeError || !tree) {
      return;
    }

    const isVisibleFile = (candidate: string | null) =>
      Boolean(candidate) && tree.entries.some((entry) => entry.kind === 'file' && entry.path === candidate);

    if (isVisibleFile(selectedFilePath)) {
      return;
    }

    if (initialPath && isVisibleFile(initialPath)) {
      setSelectedFilePath(initialPath);
      return;
    }

    if (initialPath && pathDirectory(initialPath) === tree.path) {
      const fallbackFile = tree.entries.find((entry) => entry.kind === 'file')?.path ?? null;
      setSelectedFilePath(fallbackFile);
      if (fallbackFile && fallbackFile !== initialPath) {
        onRouteChange(fallbackFile, tree.path, selectedRevision);
      }
    }
  }, [initialPath, onRouteChange, selectedFilePath, selectedRevision, tree, treeError, treeLoading]);

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
      setBlob(null);
      setBlobError(null);
      setSymbol('');
      setNavigationError(null);
      setNavigationState(null);
      onRouteChange(null, entry.path, selectedRevision);
      return;
    }

    setSelectedFilePath(entry.path);
    setSymbol('');
    setNavigationError(null);
    setNavigationState(null);
    onRouteChange(entry.path, pathDirectory(entry.path), selectedRevision);
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
    onRouteChange(target.path, pathDirectory(target.path), target.revision);
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
            <button
              type="button"
              style={secondaryButtonStyle}
              onClick={() => {
                setTreePath(parentPath);
                setSelectedFilePath(null);
                setBlob(null);
                setBlobError(null);
                setSymbol('');
                setNavigationError(null);
                setNavigationState(null);
                onRouteChange(null, parentPath, selectedRevision);
              }}
            >
              Up
            </button>
          ) : null}
        </div>

        {treeLoading ? <div>Loading files…</div> : null}
        {!treeLoading && treeError ? (
          <div style={{ display: 'grid', gap: 12 }}>
            <div>Unable to load files: {treeError}</div>
            <div>
              <button type="button" style={secondaryButtonStyle} onClick={() => setTreeRequestKey((value) => value + 1)}>
                Retry loading files
              </button>
            </div>
          </div>
        ) : null}

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

function repositorySyncJobInProgressPriority(status: RepositorySyncJobStatus) {
  if (status === 'running') {
    return 2;
  }
  if (status === 'queued') {
    return 1;
  }
  return 0;
}

function repositorySyncJobTerminalPriority(status: RepositorySyncJobStatus) {
  if (status === 'succeeded') {
    return 2;
  }
  if (status === 'failed') {
    return 1;
  }
  return 0;
}

function compareRepositorySyncJobs(left: RepositorySyncJob, right: RepositorySyncJob) {
  const queuedAtDifference = Date.parse(right.queued_at) - Date.parse(left.queued_at);
  if (queuedAtDifference !== 0) {
    return queuedAtDifference;
  }

  const activityDifference = repositorySyncJobActivityTimestamp(right) - repositorySyncJobActivityTimestamp(left);
  if (activityDifference !== 0) {
    return activityDifference;
  }

  return repositorySyncJobInProgressPriority(right.status) - repositorySyncJobInProgressPriority(left.status);
}

function compareRepositorySyncHistoryJobs(left: RepositorySyncJob, right: RepositorySyncJob) {
  const baseComparison = compareRepositorySyncJobs(left, right);
  if (baseComparison !== 0) {
    return baseComparison;
  }

  return repositorySyncJobTerminalPriority(right.status) - repositorySyncJobTerminalPriority(left.status);
}

function compareLatestRepositorySyncJobs(left: RepositorySyncJob, right: RepositorySyncJob) {
  const baseComparison = compareRepositorySyncJobs(left, right);
  if (baseComparison !== 0) {
    return baseComparison;
  }

  return repositorySyncJobInProgressPriority(right.status) - repositorySyncJobInProgressPriority(left.status);
}

function repositorySyncJobsByConnectionId(syncJobs: RepositorySyncJob[]) {
  return syncJobs.reduce<Map<string, RepositorySyncJob[]>>((jobsByConnectionId, syncJob) => {
    const existingJobs = jobsByConnectionId.get(syncJob.connection_id) ?? [];
    existingJobs.push(syncJob);
    jobsByConnectionId.set(syncJob.connection_id, existingJobs);
    return jobsByConnectionId;
  }, new Map<string, RepositorySyncJob[]>());
}

function sortedRepositorySyncHistoryJobs(syncJobs: RepositorySyncJob[]) {
  return [...syncJobs].sort(compareRepositorySyncHistoryJobs);
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
      const response = await authFetch(`/api/v1/auth/connections/${connectionId}`, {
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
            const connectionSyncHistoryJobs = sortedRepositorySyncHistoryJobs(connectionSyncJobs);
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
                  {!syncJobsError && !syncJobsLoading && connectionSyncHistoryJobs.length === 0 ? (
                    <div style={{ color: '#57606a', fontSize: 14 }}>No repository sync jobs found for this connection.</div>
                  ) : null}
                  {!syncJobsError && connectionSyncHistoryJobs.length > 0 ? (
                    <div style={{ display: 'grid', gap: 8 }}>
                      {connectionSyncHistoryJobs.map((syncJob) => (
                        <div
                          key={syncJob.id}
                          aria-label={`Repository sync history row for ${syncJob.repository_id} (${syncJob.status})`}
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
                          {syncJob.synced_revision ? <div>Revision: {syncJob.synced_revision}</div> : null}
                          {syncJob.synced_branch ? <div>Current branch: {syncJob.synced_branch}</div> : null}
                          {syncJob.synced_content_file_count != null ? (
                            <div>Tracked content files: {syncJob.synced_content_file_count}</div>
                          ) : null}
                          <a
                            href={`#/repos/${encodeURIComponent(syncJob.repository_id)}`}
                            aria-label={`Open repository detail for ${syncJob.repository_id}`}
                            style={{ color: '#0969da', fontWeight: 600 }}
                          >
                            Open repository detail
                          </a>
                          {syncJob.error ? <div aria-label={`Error details for ${syncJob.repository_id} (${syncJob.status})`}>Error: {syncJob.error}</div> : null}
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

type SettingsSectionId =
  | 'connections'
  | 'api-keys'
  | 'members'
  | 'access'
  | 'linked-accounts'
  | 'oauth-clients'
  | 'observability'
  | 'review-automation';

type SettingsSectionDefinition = {
  id: SettingsSectionId;
  label: string;
  href: string;
  description: string;
};

const settingsSections: SettingsSectionDefinition[] = [
  {
    id: 'connections',
    label: 'Connections',
    href: '#/settings/connections',
    description:
      'Inspect authenticated connection inventory and repository sync history exposed by /api/v1/auth/connections and /api/v1/auth/repository-sync-jobs.',
  },
  {
    id: 'api-keys',
    label: 'API keys',
    href: '#/settings/api-keys',
    description: 'Track the authenticated API-key lifecycle surface already exposed at /api/v1/auth/api-keys.',
  },
  {
    id: 'members',
    label: 'Members',
    href: '#/settings/members',
    description: 'Inspect the authenticated read-only organization member and invite inventory exposed at /api/v1/auth/members.',
  },
  {
    id: 'access',
    label: 'Access',
    href: '#/settings/access',
    description: 'Inspect the repositories currently visible to your account through the authz-filtered /api/v1/repos inventory.',
  },
  {
    id: 'linked-accounts',
    label: 'Linked accounts',
    href: '#/settings/linked-accounts',
    description: 'Inspect the current local account identity, same-user external identities, and visible organization memberships exposed at /api/v1/auth/linked-accounts.',
  },
  {
    id: 'oauth-clients',
    label: 'OAuth clients',
    href: '#/settings/oauth-clients',
    description: 'Discover the current OAuth client administration surface exposed at /api/v1/auth/oauth-clients.',
  },
  {
    id: 'observability',
    label: 'Audit & analytics',
    href: '#/settings/observability',
    description: 'Inspect the authenticated audit and analytics endpoints at /api/v1/auth/audit-events and /api/v1/auth/analytics.',
  },
  {
    id: 'review-automation',
    label: 'Review automation',
    href: '#/settings/review-automation',
    description: 'Point users toward authenticated review webhook, delivery-attempt, and review-agent visibility APIs.',
  },
];

function settingsSectionById(sectionId: SettingsSectionId) {
  return settingsSections.find((section) => section.id === sectionId) ?? settingsSections[0];
}

function SettingsShell({
  activeSection,
  children,
}: {
  activeSection?: SettingsSectionId;
  children: ReactNode;
}) {
  return (
    <div style={{ display: 'grid', gap: 20 }}>
      <Panel title="Settings" subtitle="Choose an authenticated admin surface to inspect.">
        <div style={settingsNavGridStyle}>
          {settingsSections.map((section) => {
            const isActive = section.id === activeSection;

            return (
              <a
                key={section.id}
                href={section.href}
                aria-current={isActive ? 'page' : undefined}
                aria-label={section.label}
                style={{
                  ...settingsNavCardStyle,
                  borderColor: isActive ? '#0969da' : '#d8dee4',
                  boxShadow: isActive ? 'inset 0 0 0 1px #0969da' : 'none',
                }}
              >
                <div style={{ fontSize: 16, fontWeight: 700, color: '#1f2328' }}>{section.label}</div>
                <div style={{ color: '#57606a', marginTop: 8 }}>{section.description}</div>
              </a>
            );
          })}
        </div>
      </Panel>
      {children}
    </div>
  );
}

function SettingsLandingPage() {
  return (
    <Panel
      title="Settings overview"
      subtitle="This shell expands settings discoverability while richer management workflows remain follow-up work."
    >
      <p style={{ margin: 0, color: '#57606a' }}>
        The current rewrite exposes authenticated admin API surfaces for connections, API keys, members, access visibility,
        linked accounts, OAuth clients, audit and analytics, and review automation visibility. Use the sections above to
        inspect the shipped route shells.
      </p>
    </Panel>
  );
}

function SettingsAccessPage() {
  const section = settingsSectionById('access');
  const [repositories, setRepositories] = useState<RepoSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<RepoSummary[]>('/api/v1/repos')
      .then((data) => {
        if (!cancelled) {
          setRepositories(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setRepositories([]);
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

  return (
    <Panel title="Access" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          The current route is read-only: it lists repositories already visible to your account so you can inspect the
          shipped access baseline without claiming permission-sync management or role-edit workflows that do not exist yet.
        </p>

        {loading ? <div>Loading visible repositories…</div> : null}
        {!loading && error ? <div>Unable to load visible repositories: {error}</div> : null}
        {!loading && !error && repositories.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No visible repositories found</div>
            <div style={{ color: '#57606a' }}>
              Your current account does not have any visible repositories yet. Permission sync and access-management
              workflows remain follow-up work.
            </div>
          </div>
        ) : null}
        {!loading && !error && repositories.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {repositories.map((repository) => (
              <li
                key={repository.id}
                aria-label={`Visible repository ${repository.name}`}
                style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
              >
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: 12,
                    alignItems: 'flex-start',
                    flexWrap: 'wrap',
                  }}
                >
                  <div style={{ display: 'grid', gap: 4 }}>
                    <div style={{ fontSize: 18, fontWeight: 700 }}>{repository.name}</div>
                    <div style={{ color: '#57606a' }}>Repository id: {repository.id}</div>
                  </div>
                  <StatusBadge state={repository.sync_state} />
                </div>
                <div style={detailGridStyle}>
                  <Detail label="Default branch" value={repository.default_branch} />
                  <Detail label="Sync state" value={<StatusBadge state={repository.sync_state} />} />
                </div>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsLinkedAccountsPage() {
  const section = settingsSectionById('linked-accounts');
  const [inventory, setInventory] = useState<LinkedAccountsInventory | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<LinkedAccountsInventory>('/api/v1/auth/linked-accounts')
      .then((data) => {
        if (!cancelled) {
          setInventory(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setInventory(null);
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

  const identities = inventory?.identities ?? [];
  const memberships = inventory?.memberships ?? [];

  return (
    <Panel title="Linked accounts" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This baseline is intentionally read-only: it shows the current local account identity, same-user external
          identities, and visible organization memberships without claiming external provider linking/callback exchange,
          SSO login, or account-merge workflows that do not exist yet.
        </p>
        {!loading && !error && inventory && !inventory.external_linking_supported ? (
          <p style={{ margin: 0, color: '#57606a' }}>
            External provider linking/callback exchange, SSO login, and account merge remain follow-up work.
          </p>
        ) : null}

        {loading ? <div>Loading linked accounts…</div> : null}
        {!loading && error ? <div>Unable to load linked accounts: {error}</div> : null}
        {!loading && !error && identities.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No linked identities found</div>
            <div style={{ color: '#57606a' }}>
              Your authenticated session does not currently map to a visible local account identity.
            </div>
          </div>
        ) : null}
        {!loading && !error && identities.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {identities.map((identity) => (
              <li
                key={`${identity.provider}-${identity.user_id}`}
                aria-label={`Linked identity ${identity.provider} ${identity.name}`}
                style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                  <div style={{ display: 'grid', gap: 4 }}>
                    <div style={{ fontSize: 18, fontWeight: 700 }}>{identity.name}</div>
                    <div style={{ color: '#57606a' }}>{identity.email}</div>
                    <div style={{ color: '#57606a' }}>User id: {identity.user_id}</div>
                  </div>
                  <span style={sharedStatusBadgeStyle(identity.primary ? '#0969da' : '#57606a')}>
                    {identity.provider}
                  </span>
                </div>
                <div style={detailGridStyle}>
                  <Detail label="Provider" value={identity.provider} />
                  <Detail label="Created at" value={identity.created_at} />
                </div>
                {identity.primary ? <div style={{ color: '#57606a' }}>Current session identity</div> : null}
              </li>
            ))}
          </ul>
        ) : null}
        {!loading && !error && memberships.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No visible memberships found</div>
            <div style={{ color: '#57606a' }}>
              Your current identity is not yet attached to any visible organization membership records.
            </div>
          </div>
        ) : null}
        {!loading && !error && memberships.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {memberships.map((membership) => (
              <li
                key={`${membership.organization.id}-${membership.joined_at}`}
                aria-label={`Linked-account membership ${membership.organization.name}`}
                style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                  <div style={{ display: 'grid', gap: 4 }}>
                    <div style={{ fontSize: 18, fontWeight: 700 }}>{membership.organization.name}</div>
                    <div style={{ color: '#57606a' }}>Organization id: {membership.organization.id}</div>
                    <div style={{ color: '#57606a' }}>Slug: {membership.organization.slug}</div>
                  </div>
                  <span style={sharedStatusBadgeStyle(membership.role === 'admin' ? '#0969da' : '#57606a')}>
                    {membership.role}
                  </span>
                </div>
                <div style={{ color: '#57606a' }}>Joined at {membership.joined_at}</div>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsMembersPage() {
  const section = settingsSectionById('members');
  const [organizations, setOrganizations] = useState<MembersOrganizationInventory[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [inviteEmails, setInviteEmails] = useState<Record<string, string>>({});
  const [inviteRoles, setInviteRoles] = useState<Record<string, 'viewer' | 'admin'>>({});
  const [inviteSubmittingOrgId, setInviteSubmittingOrgId] = useState<string | null>(null);
  const [inviteCancellingId, setInviteCancellingId] = useState<string | null>(null);
  const [memberRoleSubmittingId, setMemberRoleSubmittingId] = useState<string | null>(null);
  const [memberRemovingId, setMemberRemovingId] = useState<string | null>(null);
  const [inviteErrors, setInviteErrors] = useState<Record<string, string | null>>({});
  const [inviteSuccesses, setInviteSuccesses] = useState<Record<string, string | null>>({});

  useEffect(() => {
    let cancelled = false;

    fetchJson<MembersOrganizationInventory[]>('/api/v1/auth/members')
      .then((data) => {
        if (!cancelled) {
          setOrganizations(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setOrganizations([]);
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

  const handleCreateInvite = async (event: FormEvent<HTMLFormElement>, organization: MembersOrganizationInventory['organization']) => {
    event.preventDefault();
    const email = (inviteEmails[organization.id] ?? '').trim();
    const role = inviteRoles[organization.id] ?? 'viewer';
    if (
      !email ||
      inviteSubmittingOrgId !== null ||
      inviteCancellingId !== null ||
      memberRoleSubmittingId !== null ||
      memberRemovingId !== null
    ) {
      return;
    }

    setInviteSubmittingOrgId(organization.id);
    setInviteErrors((currentErrors) => ({ ...currentErrors, [organization.id]: null }));
    setInviteSuccesses((currentSuccesses) => ({ ...currentSuccesses, [organization.id]: null }));

    try {
      const updatedInventory = await fetchJson<MembersOrganizationInventory>('/api/v1/auth/members/invites', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          organization_id: organization.id,
          email,
          role,
        }),
      });

      setOrganizations((currentOrganizations) =>
        currentOrganizations.map((currentOrganization) =>
          currentOrganization.organization.id === updatedInventory.organization.id ? updatedInventory : currentOrganization
        )
      );
      setInviteEmails((currentEmails) => ({ ...currentEmails, [organization.id]: '' }));
      setInviteSuccesses((currentSuccesses) => ({ ...currentSuccesses, [organization.id]: `Invite created for ${email}.` }));
    } catch (err) {
      setInviteErrors((currentErrors) => ({
        ...currentErrors,
        [organization.id]: err instanceof Error ? err.message : 'Unknown error',
      }));
    } finally {
      setInviteSubmittingOrgId(null);
    }
  };

  const handleCancelInvite = async (organizationId: string, invite: InviteRosterEntry) => {
    if (
      invite.status !== 'pending' ||
      inviteSubmittingOrgId !== null ||
      inviteCancellingId !== null ||
      memberRoleSubmittingId !== null ||
      memberRemovingId !== null
    ) {
      return;
    }

    setInviteCancellingId(invite.id);
    setInviteErrors((currentErrors) => ({ ...currentErrors, [organizationId]: null }));
    setInviteSuccesses((currentSuccesses) => ({ ...currentSuccesses, [organizationId]: null }));

    try {
      const updatedInventory = await fetchJson<MembersOrganizationInventory>(
        `/api/v1/auth/members/invites/${encodeURIComponent(invite.id)}`,
        { method: 'DELETE' }
      );
      setOrganizations((currentOrganizations) =>
        currentOrganizations.map((currentOrganization) =>
          currentOrganization.organization.id === updatedInventory.organization.id ? updatedInventory : currentOrganization
        )
      );
      setInviteSuccesses((currentSuccesses) => ({
        ...currentSuccesses,
        [organizationId]: `Invite cancelled for ${invite.email}.`,
      }));
    } catch (err) {
      setInviteErrors((currentErrors) => ({
        ...currentErrors,
        [organizationId]: err instanceof Error ? err.message : 'Unknown error',
      }));
    } finally {
      setInviteCancellingId(null);
    }
  };

  const handleUpdateMemberRole = async (organizationId: string, member: MembersRosterEntry, role: 'viewer' | 'admin') => {
    if (
      role === member.role ||
      inviteSubmittingOrgId !== null ||
      inviteCancellingId !== null ||
      memberRoleSubmittingId !== null ||
      memberRemovingId !== null
    ) {
      return;
    }

    const submissionId = `${organizationId}:${member.user_id}`;
    setMemberRoleSubmittingId(submissionId);
    setInviteErrors((currentErrors) => ({ ...currentErrors, [organizationId]: null }));
    setInviteSuccesses((currentSuccesses) => ({ ...currentSuccesses, [organizationId]: null }));

    try {
      const updatedInventory = await fetchJson<MembersOrganizationInventory>(
        `/api/v1/auth/members/${encodeURIComponent(member.user_id)}/role`,
        {
          method: 'PATCH',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            organization_id: organizationId,
            role,
          }),
        }
      );
      setOrganizations((currentOrganizations) =>
        currentOrganizations.map((currentOrganization) =>
          currentOrganization.organization.id === updatedInventory.organization.id ? updatedInventory : currentOrganization
        )
      );
      setInviteSuccesses((currentSuccesses) => ({
        ...currentSuccesses,
        [organizationId]: `Updated ${member.account.email} to ${role}.`,
      }));
    } catch (err) {
      setInviteErrors((currentErrors) => ({
        ...currentErrors,
        [organizationId]: err instanceof Error ? err.message : 'Unknown error',
      }));
    } finally {
      setMemberRoleSubmittingId(null);
    }
  };

  const handleRemoveMember = async (organizationId: string, member: MembersRosterEntry) => {
    if (
      inviteSubmittingOrgId !== null ||
      inviteCancellingId !== null ||
      memberRoleSubmittingId !== null ||
      memberRemovingId !== null
    ) {
      return;
    }

    const submissionId = `${organizationId}:${member.user_id}`;
    setMemberRemovingId(submissionId);
    setInviteErrors((currentErrors) => ({ ...currentErrors, [organizationId]: null }));
    setInviteSuccesses((currentSuccesses) => ({ ...currentSuccesses, [organizationId]: null }));

    try {
      const updatedInventory = await fetchJson<MembersOrganizationInventory>(
        `/api/v1/auth/members/${encodeURIComponent(member.user_id)}`,
        {
          method: 'DELETE',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            organization_id: organizationId,
          }),
        }
      );
      setOrganizations((currentOrganizations) =>
        currentOrganizations.map((currentOrganization) =>
          currentOrganization.organization.id === updatedInventory.organization.id ? updatedInventory : currentOrganization
        )
      );
      setInviteSuccesses((currentSuccesses) => ({
        ...currentSuccesses,
        [organizationId]: `Removed ${member.account.email}.`,
      }));
    } catch (err) {
      setInviteErrors((currentErrors) => ({
        ...currentErrors,
        [organizationId]: err instanceof Error ? err.message : 'Unknown error',
      }));
    } finally {
      setMemberRemovingId(null);
    }
  };

  return (
    <Panel title="Members" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This minimal panel shows only organizations you can administer plus their current member and invite inventory,
          and it can create/cancel pending local invites, update member roles, or remove members for those administered
          organizations. Email delivery, resend, last-admin/self-removal policy, and full member lifecycle workflows remain
          follow-up work.
        </p>

        {loading ? <div>Loading members…</div> : null}
        {!loading && error ? <div>Unable to load members: {error}</div> : null}
        {!loading && !error && organizations.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No administered organizations found</div>
            <div style={{ color: '#57606a' }}>
              No member inventory is currently available for your authenticated admin scope.
            </div>
          </div>
        ) : null}
        {!loading && !error && organizations.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {organizations.map((organizationInventory) => (
              <li
                key={organizationInventory.organization.id}
                aria-label={`Organization members ${organizationInventory.organization.name}`}
                style={{ ...detailCardStyle, display: 'grid', gap: 16 }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                  <div style={{ display: 'grid', gap: 4 }}>
                    <div style={{ fontSize: 18, fontWeight: 700 }}>{organizationInventory.organization.name}</div>
                    <div style={{ color: '#57606a' }}>Organization id: {organizationInventory.organization.id}</div>
                    <div style={{ color: '#57606a' }}>Slug: {organizationInventory.organization.slug}</div>
                  </div>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, alignContent: 'flex-start' }}>
                    <span style={searchMetaBadgeStyle}>{organizationInventory.members.length} members</span>
                    <span style={searchMetaBadgeStyle}>{organizationInventory.invites.length} invites</span>
                  </div>
                </div>

                <form
                  onSubmit={(event) => handleCreateInvite(event, organizationInventory.organization)}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div style={{ fontSize: 16, fontWeight: 700 }}>Create invite</div>
                  <div style={{ color: '#57606a' }}>
                    Create a pending local invite for this organization. The current baseline records the invite for
                    redemption, can cancel pending invites, update member roles, and remove members, but does not send
                    email, resend invites, or enforce last-admin/self-removal policy.
                  </div>
                  <div style={detailGridStyle}>
                    <label style={fieldLabelStyle}>
                      <span>Invite email</span>
                      <input
                        type="email"
                        value={inviteEmails[organizationInventory.organization.id] ?? ''}
                        onChange={(event) =>
                          setInviteEmails((currentEmails) => ({
                            ...currentEmails,
                            [organizationInventory.organization.id]: event.target.value,
                          }))
                        }
                        style={inputStyle}
                        disabled={inviteSubmittingOrgId !== null || memberRoleSubmittingId !== null || memberRemovingId !== null}
                      />
                    </label>
                    <label style={fieldLabelStyle}>
                      <span>Invite role</span>
                      <select
                        value={inviteRoles[organizationInventory.organization.id] ?? 'viewer'}
                        onChange={(event) =>
                          setInviteRoles((currentRoles) => ({
                            ...currentRoles,
                            [organizationInventory.organization.id]: event.target.value as 'viewer' | 'admin',
                          }))
                        }
                        style={inputStyle}
                        disabled={inviteSubmittingOrgId !== null || memberRoleSubmittingId !== null || memberRemovingId !== null}
                      >
                        <option value="viewer">viewer</option>
                        <option value="admin">admin</option>
                      </select>
                    </label>
                  </div>
                  <button
                    type="submit"
                    style={primaryButtonStyle}
                    disabled={
                      inviteSubmittingOrgId !== null ||
                      inviteCancellingId !== null ||
                      memberRoleSubmittingId !== null ||
                      memberRemovingId !== null ||
                      (inviteEmails[organizationInventory.organization.id] ?? '').trim().length === 0
                    }
                  >
                    {inviteSubmittingOrgId === organizationInventory.organization.id
                      ? 'Creating invite…'
                      : `Create invite for ${organizationInventory.organization.name}`}
                  </button>
                  {inviteSuccesses[organizationInventory.organization.id] ? (
                    <div style={{ color: '#1a7f37' }}>{inviteSuccesses[organizationInventory.organization.id]}</div>
                  ) : null}
                  {inviteErrors[organizationInventory.organization.id] ? (
                    <div style={{ color: '#cf222e' }}>Invite create failed: {inviteErrors[organizationInventory.organization.id]}</div>
                  ) : null}
                </form>

                <div style={{ display: 'grid', gap: 10 }}>
                  <div style={{ fontSize: 16, fontWeight: 700 }}>Members</div>
                  {organizationInventory.members.length === 0 ? (
                    <div style={{ color: '#57606a' }}>No members are currently recorded for this organization.</div>
                  ) : (
                    <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 10 }}>
                      {organizationInventory.members.map((member) => (
                        <li key={`${organizationInventory.organization.id}-${member.user_id}`} style={detailCardStyle}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                            <div style={{ display: 'grid', gap: 4 }}>
                              <div style={{ fontWeight: 700 }}>{member.account.name}</div>
                              <div style={{ color: '#57606a' }}>{member.account.email}</div>
                              <div style={{ color: '#57606a' }}>User id: {member.user_id}</div>
                            </div>
                            <span style={sharedStatusBadgeStyle(member.role === 'admin' ? '#0969da' : '#57606a')}>
                              {member.role}
                            </span>
                          </div>
                          <div style={{ color: '#57606a', marginTop: 8 }}>
                            Joined at {member.joined_at}
                            {member.account.created_at ? ` · Account created ${member.account.created_at}` : ''}
                          </div>
                          <label style={{ ...fieldLabelStyle, marginTop: 12 }}>
                            <span>Update role</span>
                            <select
                              aria-label={`Role for ${member.account.email}`}
                              value={member.role}
                              onChange={(event) =>
                                handleUpdateMemberRole(
                                  organizationInventory.organization.id,
                                  member,
                                  event.target.value as 'viewer' | 'admin'
                                )
                              }
                              style={inputStyle}
                              disabled={
                                inviteSubmittingOrgId !== null ||
                                inviteCancellingId !== null ||
                                memberRoleSubmittingId !== null ||
                                memberRemovingId !== null
                              }
                            >
                              <option value="viewer">viewer</option>
                              <option value="admin">admin</option>
                            </select>
                          </label>
                          <button
                            type="button"
                            style={{ ...secondaryButtonStyle, marginTop: 12 }}
                            disabled={
                              inviteSubmittingOrgId !== null ||
                              inviteCancellingId !== null ||
                              memberRoleSubmittingId !== null ||
                              memberRemovingId !== null
                            }
                            onClick={() => handleRemoveMember(organizationInventory.organization.id, member)}
                          >
                            {memberRemovingId === `${organizationInventory.organization.id}:${member.user_id}`
                              ? `Removing ${member.account.email}…`
                              : `Remove ${member.account.email}`}
                          </button>
                          {memberRoleSubmittingId === `${organizationInventory.organization.id}:${member.user_id}` ? (
                            <div style={{ color: '#57606a', marginTop: 8 }}>Updating role for {member.account.email}…</div>
                          ) : null}
                          {memberRemovingId === `${organizationInventory.organization.id}:${member.user_id}` ? (
                            <div style={{ color: '#57606a', marginTop: 8 }}>Removing {member.account.email}…</div>
                          ) : null}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>

                <div style={{ display: 'grid', gap: 10 }}>
                  <div style={{ fontSize: 16, fontWeight: 700 }}>Invites</div>
                  {organizationInventory.invites.length === 0 ? (
                    <div style={{ color: '#57606a' }}>No invites are currently recorded for this organization.</div>
                  ) : (
                    <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 10 }}>
                      {organizationInventory.invites.map((invite) => (
                        <li key={invite.id} style={detailCardStyle}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                            <div style={{ display: 'grid', gap: 4 }}>
                              <div style={{ fontWeight: 700 }}>{invite.email}</div>
                              <div style={{ color: '#57606a' }}>Invite id: {invite.id}</div>
                              <div style={{ color: '#57606a' }}>Role: {invite.role}</div>
                            </div>
                            <span style={sharedStatusBadgeStyle(invite.status === 'accepted' ? '#1a7f37' : '#9a6700')}>
                              {invite.status}
                            </span>
                          </div>
                          <div style={{ color: '#57606a', marginTop: 8, display: 'grid', gap: 4 }}>
                            <div>Created at {invite.created_at}</div>
                            <div>Expires at {invite.expires_at}</div>
                            <div>
                              Invited by {invite.invited_by ? invite.invited_by.name : 'Unknown local account'}
                            </div>
                            <div>
                              {invite.status === 'accepted'
                                ? `Accepted by ${invite.accepted_by ? invite.accepted_by.name : 'Unknown local account'}`
                                : 'Awaiting acceptance'}
                            </div>
                            {invite.accepted_at ? <div>Accepted at {invite.accepted_at}</div> : null}
                            {invite.status === 'pending' ? (
                              <button
                                type="button"
                                style={{ ...secondaryButtonStyle, marginTop: 8 }}
                                disabled={inviteSubmittingOrgId !== null || inviteCancellingId !== null || memberRemovingId !== null}
                                onClick={() => handleCancelInvite(organizationInventory.organization.id, invite)}
                              >
                                {inviteCancellingId === invite.id
                                  ? `Cancelling invite for ${invite.email}…`
                                  : `Cancel invite for ${invite.email}`}
                              </button>
                            ) : null}
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsApiKeysPage() {
  const section = settingsSectionById('api-keys');
  const [apiKeys, setApiKeys] = useState<ApiKeyListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revokingKeyId, setRevokingKeyId] = useState<string | null>(null);
  const [revokedKeyIds, setRevokedKeyIds] = useState<string[]>([]);
  const [revokeErrors, setRevokeErrors] = useState<Record<string, string | null>>({});
  const [keyName, setKeyName] = useState('');
  const [repoScopeText, setRepoScopeText] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createdKeyResult, setCreatedKeyResult] = useState<CreateApiKeyResponse | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<ApiKeyListItem[]>('/api/v1/auth/api-keys')
      .then((data) => {
        if (!cancelled) {
          setApiKeys(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setApiKeys([]);
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

  const parsedRepoScope = repoScopeText
    .split('\n')
    .map((repoId) => repoId.trim())
    .filter((repoId) => repoId.length > 0);
  const createDisabled = loading || error !== null || isCreating || keyName.trim().length === 0;

  const handleCreateApiKey = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (createDisabled) {
      return;
    }

    setIsCreating(true);
    setCreateError(null);
    setCreatedKeyResult(null);

    try {
      const createdKey = await fetchJson<CreateApiKeyResponse>('/api/v1/auth/api-keys', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          name: keyName.trim(),
          repo_scope: parsedRepoScope,
        }),
      });

      setApiKeys((currentKeys) => [
        {
          id: createdKey.id,
          user_id: createdKey.user_id,
          name: createdKey.name,
          created_at: createdKey.created_at,
          revoked_at: createdKey.revoked_at,
          repo_scope: createdKey.repo_scope,
        },
        ...currentKeys,
      ]);
      setCreatedKeyResult(createdKey);
      setKeyName('');
      setRepoScopeText('');
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setIsCreating(false);
    }
  };

  const handleRevoke = async (apiKeyId: string) => {
    setRevokeErrors((currentErrors) => ({ ...currentErrors, [apiKeyId]: null }));
    setRevokingKeyId(apiKeyId);

    try {
      const response = await authFetch(`/api/v1/auth/api-keys/${apiKeyId}/revoke`, {
        method: 'POST',
      });

      if (!response.ok) {
        throw new Error(`Request failed: ${response.status}`);
      }

      setRevokedKeyIds((currentIds) => (currentIds.includes(apiKeyId) ? currentIds : [...currentIds, apiKeyId]));
    } catch (err) {
      setRevokeErrors((currentErrors) => ({
        ...currentErrors,
        [apiKeyId]: err instanceof Error ? err.message : 'Unknown error',
      }));
    } finally {
      setRevokingKeyId(null);
    }
  };

  return (
    <Panel title="API keys" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This minimal panel shows the authenticated inventory, lets you create a key, and lets you revoke an active key.
          Richer scoping UX, rotation, and broader credential-management workflows remain follow-up work.
        </p>

        {!loading && !error ? (
          <form onSubmit={handleCreateApiKey} style={{ ...detailCardStyle, display: 'grid', gap: 12 }}>
            <div style={{ fontSize: 18, fontWeight: 700 }}>Create API key</div>
            <div style={{ color: '#57606a' }}>
              Create a key for the current authenticated account. Leave repository scope empty to use the repos currently
              visible to you; enter one repository id per line for a bounded scoped key. The plaintext secret is returned
              only once after a successful create request.
            </div>

            <div style={detailGridStyle}>
              <label style={fieldLabelStyle}>
                <span>Key name</span>
                <input value={keyName} onChange={(event) => setKeyName(event.target.value)} style={inputStyle} disabled={isCreating} />
              </label>

              <label style={fieldLabelStyle}>
                <span>Repository scope</span>
                <textarea
                  value={repoScopeText}
                  onChange={(event) => setRepoScopeText(event.target.value)}
                  style={{ ...inputStyle, minHeight: 96, resize: 'vertical' }}
                  disabled={isCreating}
                />
              </label>
            </div>

            <div style={{ color: '#57606a' }}>Enter one repository id per line, or leave blank for current visibility.</div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
              <button type="submit" style={primaryButtonStyle} disabled={createDisabled}>
                {isCreating ? 'Creating…' : 'Create API key'}
              </button>
              {createError ? <div>Unable to create API key: {createError}</div> : null}
            </div>
          </form>
        ) : null}

        {createdKeyResult ? (
          <div style={{ ...detailCardStyle, display: 'grid', gap: 8 }}>
            <div style={{ fontWeight: 700 }}>API key created. Copy the secret now; it will not be shown again.</div>
            <Detail label="Name" value={createdKeyResult.name} />
            <Detail label="Key id" value={createdKeyResult.id} />
            <Detail label="Secret" value={createdKeyResult.secret} />
          </div>
        ) : null}

        {loading ? <div>Loading API keys…</div> : null}
        {!loading && error ? <div>Unable to load API keys: {error}</div> : null}
        {!loading && !error && apiKeys.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No API keys found</div>
            <div style={{ color: '#57606a' }}>
              Your account does not have any API keys yet. Use the create form above for the current minimal flow; richer
              rotation, bulk management, and advanced scoping workflows remain follow-up work.
            </div>
          </div>
        ) : null}
        {!loading && !error && apiKeys.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {apiKeys.map((apiKey) => {
              const isRevoked = apiKey.revoked_at !== null || revokedKeyIds.includes(apiKey.id);
              const isRevoking = revokingKeyId === apiKey.id;

              return (
                <li
                  key={apiKey.id}
                  aria-label={`API key ${apiKey.name}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{apiKey.name}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Key id: {apiKey.id}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle(isRevoked ? '#cf222e' : '#1a7f37')}>
                      {isRevoked ? 'revoked' : 'active'}
                    </span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="User id" value={apiKey.user_id} />
                    <Detail label="Created at" value={apiKey.created_at} />
                    <Detail
                      label="Revoked at"
                      value={
                        apiKey.revoked_at
                          ? apiKey.revoked_at
                          : isRevoked
                            ? 'Revoked successfully. Refresh later to load the server timestamp.'
                            : 'Active'
                      }
                    />
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Repo scope</div>
                    {apiKey.repo_scope.length === 0 ? (
                      <div style={{ color: '#57606a' }}>
                        Not repo-bound. This key can reach the repos currently visible to you.
                      </div>
                    ) : (
                      <div style={{ display: 'grid', gap: 8 }}>
                        <div style={{ color: '#57606a' }}>Limited to the listed repositories.</div>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                          {apiKey.repo_scope.map((repoId) => (
                            <span key={repoId} style={searchMetaBadgeStyle}>
                              {repoId}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    {!isRevoked ? (
                      <div>
                        <button
                          type="button"
                          style={secondaryButtonStyle}
                          disabled={isRevoking}
                          onClick={() => {
                            void handleRevoke(apiKey.id);
                          }}
                          aria-label={`Revoke key ${apiKey.name}`}
                        >
                          {isRevoking ? 'Revoking…' : 'Revoke key'}
                        </button>
                      </div>
                    ) : null}
                    {revokeErrors[apiKey.id] ? <div>Unable to revoke key: {revokeErrors[apiKey.id]}</div> : null}
                  </div>
                </li>
              );
            })}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsOAuthClientsPage() {
  const section = settingsSectionById('oauth-clients');
  const [oauthClients, setOauthClients] = useState<OAuthClientListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [organizationId, setOrganizationId] = useState('');
  const [clientName, setClientName] = useState('');
  const [redirectUrisText, setRedirectUrisText] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createdClientResult, setCreatedClientResult] = useState<CreateOAuthClientResponse | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<OAuthClientListItem[]>('/api/v1/auth/oauth-clients')
      .then((data) => {
        if (!cancelled) {
          setOauthClients(data);
          setError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setOauthClients([]);
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

  const parsedRedirectUris = redirectUrisText
    .split('\n')
    .map((redirectUri) => redirectUri.trim())
    .filter((redirectUri) => redirectUri.length > 0);
  const createDisabled =
    loading ||
    error !== null ||
    isCreating ||
    organizationId.trim().length === 0 ||
    clientName.trim().length === 0 ||
    parsedRedirectUris.length === 0;

  const handleCreateOAuthClient = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (createDisabled) {
      return;
    }

    setIsCreating(true);
    setCreateError(null);
    setCreatedClientResult(null);

    try {
      const createdClient = await fetchJson<CreateOAuthClientResponse>('/api/v1/auth/oauth-clients', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          organization_id: organizationId.trim(),
          name: clientName.trim(),
          redirect_uris: parsedRedirectUris,
        }),
      });

      setOauthClients((currentClients) => [
        {
          id: createdClient.id,
          organization_id: createdClient.organization_id,
          name: createdClient.name,
          client_id: createdClient.client_id,
          redirect_uris: createdClient.redirect_uris,
          created_by_user_id: createdClient.created_by_user_id,
          created_at: createdClient.created_at,
          revoked_at: createdClient.revoked_at,
        },
        ...currentClients,
      ]);
      setCreatedClientResult(createdClient);
      setOrganizationId('');
      setClientName('');
      setRedirectUrisText('');
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <Panel title="OAuth clients" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This minimal panel shows the authenticated visible OAuth client inventory plus a focused create form. Richer OAuth
          authorization, token issuance and revocation, and broader manage UX remain follow-up work.
        </p>

        {!loading && !error ? (
          <form onSubmit={handleCreateOAuthClient} style={{ ...detailCardStyle, display: 'grid', gap: 12 }}>
            <div style={{ fontSize: 18, fontWeight: 700 }}>Create OAuth client</div>
            <div style={{ color: '#57606a' }}>
              Create a client for an organization you can administer. The plaintext client secret is returned only once after a
              successful create request.
            </div>

            <div style={detailGridStyle}>
              <label style={fieldLabelStyle}>
                <span>Organization id</span>
                <input
                  value={organizationId}
                  onChange={(event) => setOrganizationId(event.target.value)}
                  style={inputStyle}
                  disabled={isCreating}
                />
              </label>

              <label style={fieldLabelStyle}>
                <span>Client name</span>
                <input
                  value={clientName}
                  onChange={(event) => setClientName(event.target.value)}
                  style={inputStyle}
                  disabled={isCreating}
                />
              </label>
            </div>

            <label style={fieldLabelStyle}>
              <span>Redirect URIs</span>
              <textarea
                value={redirectUrisText}
                onChange={(event) => setRedirectUrisText(event.target.value)}
                style={{ ...inputStyle, minHeight: 96, resize: 'vertical' }}
                disabled={isCreating}
              />
            </label>

            <div style={{ color: '#57606a' }}>Enter one redirect URI per line.</div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
              <button type="submit" style={primaryButtonStyle} disabled={createDisabled}>
                {isCreating ? 'Creating…' : 'Create OAuth client'}
              </button>
              {createError ? <div>Unable to create OAuth client: {createError}</div> : null}
            </div>
          </form>
        ) : null}

        {createdClientResult ? (
          <div style={{ ...detailCardStyle, borderColor: '#1a7f37', background: '#eefbf1', display: 'grid', gap: 8 }}>
            <div style={{ fontWeight: 700 }}>OAuth client created. Copy the secret now.</div>
            <div>Client id: {createdClientResult.client_id}</div>
            <div>Client secret: {createdClientResult.client_secret}</div>
            <div style={{ color: '#57606a' }}>
              This secret is shown only for the immediate create result. The inventory below does not retain plaintext secret
              material.
            </div>
          </div>
        ) : null}

        {loading ? <div>Loading OAuth clients…</div> : null}
        {!loading && error ? <div>Unable to load OAuth clients: {error}</div> : null}
        {!loading && !error && oauthClients.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No OAuth clients found</div>
            <div style={{ color: '#57606a' }}>
              No visible OAuth clients are currently available for your authenticated organizations.
            </div>
          </div>
        ) : null}
        {!loading && !error && oauthClients.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {oauthClients.map((oauthClient) => {
              const isRevoked = oauthClient.revoked_at !== null;

              return (
                <li
                  key={oauthClient.id}
                  aria-label={`OAuth client ${oauthClient.name}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{oauthClient.name}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Client id: {oauthClient.client_id}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle(isRevoked ? '#cf222e' : '#1a7f37')}>
                      {isRevoked ? 'revoked' : 'active'}
                    </span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={oauthClient.organization_id} />
                    <Detail label="Created by user" value={oauthClient.created_by_user_id} />
                    <Detail label="Created at" value={oauthClient.created_at} />
                    <Detail label="Revoked at" value={oauthClient.revoked_at ?? 'Active'} />
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Redirect URIs</div>
                    {oauthClient.redirect_uris.length === 0 ? (
                      <div style={{ color: '#57606a' }}>
                        This visible OAuth client does not currently list any redirect URIs.
                      </div>
                    ) : (
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                        {oauthClient.redirect_uris.map((redirectUri) => (
                          <span key={redirectUri} style={searchMetaBadgeStyle}>
                            {redirectUri}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsObservabilityPage() {
  const section = settingsSectionById('observability');
  const [auditEvents, setAuditEvents] = useState<AuditEventListItem[]>([]);
  const [analyticsRecords, setAnalyticsRecords] = useState<AnalyticsRecordListItem[]>([]);
  const [auditLoading, setAuditLoading] = useState(true);
  const [analyticsLoading, setAnalyticsLoading] = useState(true);
  const [auditError, setAuditError] = useState<string | null>(null);
  const [analyticsError, setAnalyticsError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<AuditEventListItem[]>('/api/v1/auth/audit-events')
      .then((data) => {
        if (!cancelled) {
          setAuditEvents(data);
          setAuditError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setAuditEvents([]);
          setAuditError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAuditLoading(false);
        }
      });

    fetchJson<AnalyticsRecordListItem[]>('/api/v1/auth/analytics')
      .then((data) => {
        if (!cancelled) {
          setAnalyticsRecords(data);
          setAnalyticsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setAnalyticsRecords([]);
          setAnalyticsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAnalyticsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Panel title="Observability" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This minimal panel turns the observability route into a real authenticated operator surface by loading visible audit
          events and analytics records side by side. Richer filtering, drill-down, and export workflows remain follow-up work.
        </p>

        <div style={detailGridStyle}>
          <Detail
            label="Visible audit events"
            value={auditLoading ? 'Loading…' : auditError ? 'Unavailable' : String(auditEvents.length)}
          />
          <Detail
            label="Visible analytics metrics"
            value={analyticsLoading ? 'Loading…' : analyticsError ? 'Unavailable' : String(analyticsRecords.length)}
          />
        </div>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>Audit events</div>
          {auditLoading ? <div>Loading audit events…</div> : null}
          {!auditLoading && auditError ? <div>Unable to load audit events: {auditError}</div> : null}
          {!auditLoading && !auditError && auditEvents.length === 0 ? (
            <div style={detailCardStyle}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>No audit events found</div>
              <div style={{ color: '#57606a' }}>
                No visible audit events are currently available for your authenticated organizations.
              </div>
            </div>
          ) : null}
          {!auditLoading && !auditError && auditEvents.length > 0 ? (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
              {auditEvents.map((auditEvent) => (
                <li
                  key={auditEvent.id}
                  aria-label={`Audit event ${auditEvent.action}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{auditEvent.action}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>
                        {auditEvent.target_type}: {auditEvent.target_id}
                      </div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#0969da')}>{auditEvent.target_type}</span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={auditEvent.organization_id} />
                    <Detail label="Occurred at" value={auditEvent.occurred_at} />
                    <Detail label="User actor" value={auditEvent.actor.user_id ?? 'System / unknown'} />
                    <Detail label="API key actor" value={auditEvent.actor.api_key_id ?? 'None'} />
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Metadata</div>
                    <pre style={diffPatchStyle}>{formatJsonValue(auditEvent.metadata)}</pre>
                  </div>
                </li>
              ))}
            </ul>
          ) : null}
        </div>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>Analytics</div>
          {analyticsLoading ? <div>Loading analytics…</div> : null}
          {!analyticsLoading && analyticsError ? <div>Unable to load analytics: {analyticsError}</div> : null}
          {!analyticsLoading && !analyticsError && analyticsRecords.length === 0 ? (
            <div style={detailCardStyle}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>No analytics records found</div>
              <div style={{ color: '#57606a' }}>
                No visible analytics metrics are currently available for your authenticated organizations.
              </div>
            </div>
          ) : null}
          {!analyticsLoading && !analyticsError && analyticsRecords.length > 0 ? (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
              {analyticsRecords.map((record) => (
                <li
                  key={record.id}
                  aria-label={`Analytics metric ${record.metric}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{record.metric}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Record id: {record.id}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#1a7f37')}>metric</span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={record.organization_id} />
                    <Detail label="Recorded at" value={record.recorded_at} />
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Value</div>
                    <pre style={diffPatchStyle}>{formatJsonValue(record.value)}</pre>
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Dimensions</div>
                    <pre style={diffPatchStyle}>{formatJsonValue(record.dimensions)}</pre>
                  </div>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      </div>
    </Panel>
  );
}

function AgentsPage({ initialRunId }: { initialRunId: string | null }) {
  const [reviewAgentRuns, setReviewAgentRuns] = useState<ReviewAgentRunListItem[]>([]);
  const [runsLoading, setRunsLoading] = useState(true);
  const [runsError, setRunsError] = useState<string | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(initialRunId);
  const [selectedRunReloadKey, setSelectedRunReloadKey] = useState(0);
  const [selectedRun, setSelectedRun] = useState<ReviewAgentRunDetail | null>(null);
  const [selectedRunLoading, setSelectedRunLoading] = useState(false);
  const [selectedRunError, setSelectedRunError] = useState<string | null>(null);
  const [selectedDeliveryAttempt, setSelectedDeliveryAttempt] = useState<ReviewWebhookDeliveryAttemptDetail | null>(null);
  const [selectedDeliveryAttemptLoading, setSelectedDeliveryAttemptLoading] = useState(false);
  const [selectedDeliveryAttemptError, setSelectedDeliveryAttemptError] = useState<string | null>(null);
  const [selectedWebhook, setSelectedWebhook] = useState<ReviewWebhookDetail | null>(null);
  const [selectedWebhookLoading, setSelectedWebhookLoading] = useState(false);
  const [selectedWebhookError, setSelectedWebhookError] = useState<string | null>(null);

  useEffect(() => {
    setSelectedRunId(initialRunId);
  }, [initialRunId]);

  useEffect(() => {
    let cancelled = false;

    fetchJson<ReviewAgentRunListItem[]>('/api/v1/auth/review-agent-runs')
      .then((data) => {
        if (!cancelled) {
          setReviewAgentRuns(data);
          setRunsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setReviewAgentRuns([]);
          setRunsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setRunsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    setSelectedRun(null);
    setSelectedDeliveryAttempt(null);
    setSelectedDeliveryAttemptError(null);
    setSelectedDeliveryAttemptLoading(false);
    setSelectedWebhook(null);
    setSelectedWebhookError(null);
    setSelectedWebhookLoading(false);

    if (!selectedRunId) {
      setSelectedRunLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setSelectedRunError(null);
    setSelectedRunLoading(true);

    fetchJson<ReviewAgentRunDetail>(`/api/v1/auth/review-agent-runs/${encodeURIComponent(selectedRunId)}`)
      .then((data) => {
        if (cancelled) {
          return;
        }

        setSelectedRun(data);
        setSelectedRunError(null);

        setSelectedDeliveryAttemptLoading(true);
        fetchJson<ReviewWebhookDeliveryAttemptDetail>(
          `/api/v1/auth/review-webhook-delivery-attempts/${encodeURIComponent(data.delivery_attempt_id)}`
        )
          .then((attempt) => {
            if (!cancelled) {
              setSelectedDeliveryAttempt(attempt);
              setSelectedDeliveryAttemptError(null);
            }
          })
          .catch((err: Error) => {
            if (!cancelled) {
              setSelectedDeliveryAttempt(null);
              setSelectedDeliveryAttemptError(err.message);
            }
          })
          .finally(() => {
            if (!cancelled) {
              setSelectedDeliveryAttemptLoading(false);
            }
          });

        setSelectedWebhookLoading(true);
        fetchJson<ReviewWebhookDetail>(`/api/v1/auth/review-webhooks/${encodeURIComponent(data.webhook_id)}`)
          .then((webhook) => {
            if (!cancelled) {
              setSelectedWebhook(webhook);
              setSelectedWebhookError(null);
            }
          })
          .catch((err: Error) => {
            if (!cancelled) {
              setSelectedWebhook(null);
              setSelectedWebhookError(err.message);
            }
          })
          .finally(() => {
            if (!cancelled) {
              setSelectedWebhookLoading(false);
            }
          });
      })
      .catch((err: Error) => {
        if (cancelled) {
          return;
        }

        if (err instanceof HttpError && err.status === 404) {
          setSelectedRunId(null);
          setSelectedRun(null);
          setSelectedRunError('The restored agent run is no longer visible.');
          window.location.hash = buildAgentsHash();
          return;
        }

        setSelectedRun(null);
        setSelectedRunError(err.message);
      })
      .finally(() => {
        if (!cancelled) {
          setSelectedRunLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedRunId, selectedRunReloadKey]);

  const handleRunSelection = (runId: string) => {
    if (selectedRunId === runId) {
      setSelectedRunReloadKey((current) => current + 1);
    } else {
      setSelectedRunId(runId);
    }
    const targetHash = buildAgentsHash(runId);
    if (window.location.hash !== targetHash) {
      window.location.hash = targetHash;
    }
  };

  return (
    <Panel title="Agents" subtitle="Inspect visible review-agent runs from a dedicated operator route.">
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This baseline promotes operator-visible review-agent runs onto their own route while richer agent management,
          retries, and orchestration remain follow-up work.
        </p>

        <div style={detailGridStyle}>
          <Detail label="Visible review-agent runs" value={runsLoading ? 'Loading…' : runsError ? 'Unavailable' : String(reviewAgentRuns.length)} />
          <Detail label="Selected run" value={selectedRunLoading ? 'Loading…' : selectedRun ? selectedRun.id : 'None'} />
          <Detail
            label="Related resources"
            value={selectedRun ? `${selectedDeliveryAttempt ? 'Delivery attempt' : 'Run only'}${selectedWebhook ? ' + webhook' : ''}` : 'Select a run'}
          />
        </div>

        {runsLoading ? <div>Loading review-agent runs…</div> : null}
        {!runsLoading && runsError ? <div>Unable to load review-agent runs: {runsError}</div> : null}
        {!runsLoading && !runsError && reviewAgentRuns.length === 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No review-agent runs found</div>
            <div style={{ color: '#57606a' }}>
              No visible review-agent runs are currently available for your authenticated organizations.
            </div>
          </div>
        ) : null}
        {!runsLoading && !runsError && reviewAgentRuns.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
            {reviewAgentRuns.map((run) => {
              const isSelected = run.id === selectedRunId;

              return (
                <li
                  key={run.id}
                  aria-label={`Agent run ${run.id}`}
                  style={{
                    ...detailCardStyle,
                    display: 'grid',
                    gap: 12,
                    borderColor: isSelected ? '#0969da' : '#d8dee4',
                    boxShadow: isSelected ? 'inset 0 0 0 1px #0969da' : 'none',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{run.id}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Review id: {run.review_id}</div>
                    </div>
                    <ReviewAgentRunStatusBadge status={run.status} />
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={run.organization_id} />
                    <Detail label="Webhook id" value={run.webhook_id} />
                    <Detail label="Delivery attempt id" value={run.delivery_attempt_id} />
                    <Detail label="Created at" value={run.created_at} />
                  </div>

                  <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
                    {isSelected ? <span style={sharedStatusBadgeStyle('#0969da')}>Selected run</span> : null}
                    <button type="button" style={primaryButtonStyle} onClick={() => handleRunSelection(run.id)}>
                      {isSelected ? 'Reload details' : 'Open run details'}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        ) : null}

        {selectedRunId && selectedRunLoading ? <div>Loading selected agent run…</div> : null}
        {!selectedRunLoading && selectedRunError ? <div>{selectedRunError}</div> : null}
        {!selectedRunId && !selectedRunError && !runsLoading && !runsError && reviewAgentRuns.length > 0 ? (
          <div style={detailCardStyle}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>No run selected</div>
            <div style={{ color: '#57606a' }}>Choose a visible review-agent run to inspect its related delivery attempt and webhook.</div>
          </div>
        ) : null}
        {!selectedRunLoading && selectedRun ? (
          <div style={{ display: 'grid', gap: 12 }}>
            <div style={{ fontSize: 18, fontWeight: 700 }}>Selected run detail</div>
            <article aria-label={`Selected agent run ${selectedRun.id}`} style={{ ...detailCardStyle, display: 'grid', gap: 12 }}>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  gap: 12,
                  alignItems: 'flex-start',
                  flexWrap: 'wrap',
                }}
              >
                <div>
                  <div style={{ fontSize: 18, fontWeight: 700 }}>{selectedRun.id}</div>
                  <div style={{ color: '#57606a', marginTop: 4 }}>Review id: {selectedRun.review_id}</div>
                </div>
                <ReviewAgentRunStatusBadge status={selectedRun.status} />
              </div>
              <div style={detailGridStyle}>
                <Detail label="Organization id" value={selectedRun.organization_id} />
                <Detail label="Webhook id" value={selectedRun.webhook_id} />
                <Detail label="Delivery attempt id" value={selectedRun.delivery_attempt_id} />
                <Detail label="Connection id" value={selectedRun.connection_id} />
                <Detail label="Repository id" value={selectedRun.repository_id} />
                <Detail label="Created at" value={selectedRun.created_at} />
              </div>
              <div>
                <span style={sharedStatusBadgeStyle('#0969da')}>Selected run</span>
              </div>
            </article>
          </div>
        ) : null}

        {selectedRun ? (
          <div style={{ display: 'grid', gap: 16 }}>
            <div style={{ display: 'grid', gap: 12 }}>
              <div style={{ fontSize: 18, fontWeight: 700 }}>Related delivery attempt</div>
              {selectedDeliveryAttemptLoading ? <div>Loading selected delivery attempt…</div> : null}
              {!selectedDeliveryAttemptLoading && selectedDeliveryAttemptError ? (
                <div>Unable to load selected delivery attempt: {selectedDeliveryAttemptError}</div>
              ) : null}
              {!selectedDeliveryAttemptLoading && selectedDeliveryAttempt ? (
                <article
                  aria-label={`Selected delivery attempt ${selectedDeliveryAttempt.id}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{selectedDeliveryAttempt.id}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Event type: {selectedDeliveryAttempt.event_type}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#8250df')}>delivery attempt</span>
                  </div>
                  <div style={detailGridStyle}>
                    <Detail label="Webhook id" value={selectedDeliveryAttempt.webhook_id} />
                    <Detail label="Connection id" value={selectedDeliveryAttempt.connection_id} />
                    <Detail label="Repository id" value={selectedDeliveryAttempt.repository_id} />
                    <Detail label="Review id" value={selectedDeliveryAttempt.review_id} />
                    <Detail label="External event id" value={selectedDeliveryAttempt.external_event_id} />
                    <Detail label="Accepted at" value={selectedDeliveryAttempt.accepted_at} />
                  </div>
                </article>
              ) : null}
            </div>

            <div style={{ display: 'grid', gap: 12 }}>
              <div style={{ fontSize: 18, fontWeight: 700 }}>Related review webhook</div>
              {selectedWebhookLoading ? <div>Loading selected review webhook…</div> : null}
              {!selectedWebhookLoading && selectedWebhookError ? <div>Unable to load selected review webhook: {selectedWebhookError}</div> : null}
              {!selectedWebhookLoading && selectedWebhook ? (
                <article aria-label={`Selected review webhook ${selectedWebhook.id}`} style={{ ...detailCardStyle, display: 'grid', gap: 12 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{selectedWebhook.id}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#0969da')}>webhook</span>
                  </div>
                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={selectedWebhook.organization_id} />
                    <Detail label="Repository id" value={selectedWebhook.repository_id} />
                    <Detail label="Connection id" value={selectedWebhook.connection_id} />
                    <Detail label="Created by user" value={selectedWebhook.created_by_user_id} />
                    <Detail label="Created at" value={selectedWebhook.created_at} />
                  </div>
                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Events</div>
                    {selectedWebhook.events.length === 0 ? (
                      <div style={{ color: '#57606a' }}>This visible review webhook does not currently list any subscribed events.</div>
                    ) : (
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                        {selectedWebhook.events.map((eventName) => (
                          <span key={eventName} style={searchMetaBadgeStyle}>
                            {eventName}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </article>
              ) : null}
            </div>
          </div>
        ) : null}
      </div>
    </Panel>
  );
}

function SettingsReviewAutomationPage() {
  const section = settingsSectionById('review-automation');
  const [reviewWebhooks, setReviewWebhooks] = useState<ReviewWebhookListItem[]>([]);
  const [deliveryAttempts, setDeliveryAttempts] = useState<ReviewWebhookDeliveryAttemptListItem[]>([]);
  const [reviewAgentRuns, setReviewAgentRuns] = useState<ReviewAgentRunListItem[]>([]);
  const [webhooksLoading, setWebhooksLoading] = useState(true);
  const [deliveryAttemptsLoading, setDeliveryAttemptsLoading] = useState(true);
  const [reviewAgentRunsLoading, setReviewAgentRunsLoading] = useState(true);
  const [webhooksError, setWebhooksError] = useState<string | null>(null);
  const [deliveryAttemptsError, setDeliveryAttemptsError] = useState<string | null>(null);
  const [reviewAgentRunsError, setReviewAgentRunsError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchJson<ReviewWebhookListItem[]>('/api/v1/auth/review-webhooks')
      .then((data) => {
        if (!cancelled) {
          setReviewWebhooks(data);
          setWebhooksError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setReviewWebhooks([]);
          setWebhooksError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setWebhooksLoading(false);
        }
      });

    fetchJson<ReviewWebhookDeliveryAttemptListItem[]>('/api/v1/auth/review-webhook-delivery-attempts')
      .then((data) => {
        if (!cancelled) {
          setDeliveryAttempts(data);
          setDeliveryAttemptsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setDeliveryAttempts([]);
          setDeliveryAttemptsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setDeliveryAttemptsLoading(false);
        }
      });

    fetchJson<ReviewAgentRunListItem[]>('/api/v1/auth/review-agent-runs')
      .then((data) => {
        if (!cancelled) {
          setReviewAgentRuns(data);
          setReviewAgentRunsError(null);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setReviewAgentRuns([]);
          setReviewAgentRunsError(err.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setReviewAgentRunsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Panel title="Review automation" subtitle={section.description}>
      <div style={{ display: 'grid', gap: 16 }}>
        <p style={{ margin: 0, color: '#57606a' }}>
          This minimal panel turns the review-automation route into a real authenticated visibility surface by loading visible review
          webhooks, delivery attempts, and review-agent runs side by side. Richer webhook management, retry, and automation run UX
          remain follow-up work.
        </p>

        <div style={detailGridStyle}>
          <Detail
            label="Visible review webhooks"
            value={webhooksLoading ? 'Loading…' : webhooksError ? 'Unavailable' : String(reviewWebhooks.length)}
          />
          <Detail
            label="Visible delivery attempts"
            value={deliveryAttemptsLoading ? 'Loading…' : deliveryAttemptsError ? 'Unavailable' : String(deliveryAttempts.length)}
          />
          <Detail
            label="Visible review-agent runs"
            value={reviewAgentRunsLoading ? 'Loading…' : reviewAgentRunsError ? 'Unavailable' : String(reviewAgentRuns.length)}
          />
        </div>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>Review webhooks</div>
          {webhooksLoading ? <div>Loading review webhooks…</div> : null}
          {!webhooksLoading && webhooksError ? <div>Unable to load review webhooks: {webhooksError}</div> : null}
          {!webhooksLoading && !webhooksError && reviewWebhooks.length === 0 ? (
            <div style={detailCardStyle}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>No review webhooks found</div>
              <div style={{ color: '#57606a' }}>
                No visible review webhooks are currently available for your authenticated organizations.
              </div>
            </div>
          ) : null}
          {!webhooksLoading && !webhooksError && reviewWebhooks.length > 0 ? (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
              {reviewWebhooks.map((webhook) => (
                <li
                  key={webhook.id}
                  aria-label={`Review webhook ${webhook.id}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{webhook.id}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#0969da')}>webhook</span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={webhook.organization_id} />
                    <Detail label="Repository id" value={webhook.repository_id} />
                    <Detail label="Connection id" value={webhook.connection_id} />
                    <Detail label="Created by user" value={webhook.created_by_user_id} />
                    <Detail label="Created at" value={webhook.created_at} />
                  </div>

                  <div style={{ display: 'grid', gap: 8 }}>
                    <div style={{ fontSize: 14, fontWeight: 700 }}>Events</div>
                    {webhook.events.length === 0 ? (
                      <div style={{ color: '#57606a' }}>This visible review webhook does not currently list any subscribed events.</div>
                    ) : (
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                        {webhook.events.map((eventName) => (
                          <span key={eventName} style={searchMetaBadgeStyle}>
                            {eventName}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          ) : null}
        </div>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>Delivery attempts</div>
          {deliveryAttemptsLoading ? <div>Loading delivery attempts…</div> : null}
          {!deliveryAttemptsLoading && deliveryAttemptsError ? (
            <div>Unable to load delivery attempts: {deliveryAttemptsError}</div>
          ) : null}
          {!deliveryAttemptsLoading && !deliveryAttemptsError && deliveryAttempts.length === 0 ? (
            <div style={detailCardStyle}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>No delivery attempts found</div>
              <div style={{ color: '#57606a' }}>
                No visible review-webhook delivery attempts are currently available for your authenticated organizations.
              </div>
            </div>
          ) : null}
          {!deliveryAttemptsLoading && !deliveryAttemptsError && deliveryAttempts.length > 0 ? (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
              {deliveryAttempts.map((attempt) => (
                <li
                  key={attempt.id}
                  aria-label={`Delivery attempt ${attempt.id}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{attempt.id}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Event type: {attempt.event_type}</div>
                    </div>
                    <span style={sharedStatusBadgeStyle('#8250df')}>delivery attempt</span>
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Webhook id" value={attempt.webhook_id} />
                    <Detail label="Connection id" value={attempt.connection_id} />
                    <Detail label="Repository id" value={attempt.repository_id} />
                    <Detail label="Review id" value={attempt.review_id} />
                    <Detail label="External event id" value={attempt.external_event_id} />
                    <Detail label="Accepted at" value={attempt.accepted_at} />
                  </div>
                </li>
              ))}
            </ul>
          ) : null}
        </div>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>Review-agent runs</div>
          {reviewAgentRunsLoading ? <div>Loading review-agent runs…</div> : null}
          {!reviewAgentRunsLoading && reviewAgentRunsError ? (
            <div>Unable to load review-agent runs: {reviewAgentRunsError}</div>
          ) : null}
          {!reviewAgentRunsLoading && !reviewAgentRunsError && reviewAgentRuns.length === 0 ? (
            <div style={detailCardStyle}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>No review-agent runs found</div>
              <div style={{ color: '#57606a' }}>
                No visible review-agent runs are currently available for your authenticated organizations.
              </div>
            </div>
          ) : null}
          {!reviewAgentRunsLoading && !reviewAgentRunsError && reviewAgentRuns.length > 0 ? (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 12 }}>
              {reviewAgentRuns.map((run) => (
                <li
                  key={run.id}
                  aria-label={`Review-agent run ${run.id}`}
                  style={{ ...detailCardStyle, display: 'grid', gap: 12 }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 12,
                      alignItems: 'flex-start',
                      flexWrap: 'wrap',
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{run.id}</div>
                      <div style={{ color: '#57606a', marginTop: 4 }}>Review id: {run.review_id}</div>
                    </div>
                    <ReviewAgentRunStatusBadge status={run.status} />
                  </div>

                  <div style={detailGridStyle}>
                    <Detail label="Organization id" value={run.organization_id} />
                    <Detail label="Webhook id" value={run.webhook_id} />
                    <Detail label="Delivery attempt id" value={run.delivery_attempt_id} />
                    <Detail label="Connection id" value={run.connection_id} />
                    <Detail label="Repository id" value={run.repository_id} />
                    <Detail label="Created at" value={run.created_at} />
                  </div>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      </div>
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

const settingsNavGridStyle: CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
  gap: 12,
};

const settingsNavCardStyle: CSSProperties = {
  padding: 16,
  borderRadius: 12,
  border: '1px solid #d8dee4',
  background: '#fff',
  color: 'inherit',
  textDecoration: 'none',
};

export function App() {
  const hash = useHashLocation();

  const route = useMemo(() => {
    const [hashPath, hashQuery = ''] = hash.split('?');

    if (hashPath === '#/auth') {
      const params = new URLSearchParams(hashQuery);
      return {
        kind: 'auth' as const,
        inviteId: params.get('invite')?.trim() || null,
        inviteEmail: params.get('email')?.trim() || null,
        oauthCallback: {
          provider: params.get('provider')?.trim() || null,
          error: params.get('error')?.trim() || null,
          errorDescription: params.get('error_description')?.trim() || null,
          code: params.get('code')?.trim() || null,
          state: params.get('state')?.trim() || null,
        },
      };
    }

    if (hashPath === '#/search') {
      const params = new URLSearchParams(hashQuery);
      return {
        kind: 'search' as const,
        initialQuery: params.get('q') ?? '',
        initialRepoId: params.get('repo_id') ?? '',
      };
    }

    if (hashPath === '#/ask') {
      const params = new URLSearchParams(hashQuery);
      return {
        kind: 'ask' as const,
        initialRepoId: params.get('repo_id') ?? '',
        initialThreadId: params.get('thread_id')?.trim() || null,
      };
    }

    if (hashPath === '#/chat') {
      const params = new URLSearchParams(hashQuery);
      return {
        kind: 'chat' as const,
        initialRepoId: params.get('repo_id') ?? '',
        initialThreadId: params.get('thread_id')?.trim() || null,
      };
    }

    if (hashPath === '#/agents') {
      const params = new URLSearchParams(hashQuery);
      return {
        kind: 'agents' as const,
        initialRunId: params.get('run_id')?.trim() || null,
      };
    }

    const settingsMatch = hashPath.match(/^#\/settings(?:\/([a-z-]+))?$/);
    if (settingsMatch) {
      const section = settingsMatch[1] as SettingsSectionId | undefined;
      if (!section) {
        return { kind: 'settings-landing' as const };
      }

      if (settingsSections.some((settingsSection) => settingsSection.id === section)) {
        return { kind: 'settings-section' as const, section };
      }
    }

    const match = hashPath.match(/^#\/repos\/([^/]+)$/);
    if (match) {
      const params = new URLSearchParams(hashQuery);
      const initialPath = params.get('path');
      const initialTreePath = params.get('tree_path');
      const searchQuery = params.get('q') ?? '';
      const searchRepoId = params.get('repo_id') ?? '';
      return {
        kind: 'repo' as const,
        repoId: decodeURIComponent(match[1]),
        initialPath: initialPath && initialPath.length > 0 ? initialPath : null,
        initialTreePath: initialTreePath && initialTreePath.length > 0 ? initialTreePath : null,
        initialRevision: params.get('revision')?.trim() ? params.get('revision') : null,
        from: params.get('from') === 'search' ? ('search' as const) : null,
        searchHash: buildSearchHash(searchQuery, searchRepoId),
      };
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
          Clean-room code intelligence workspace: repository inventory, ask, search, first-run onboarding/local auth, sync state, and API-backed detail views.
        </p>
        <nav style={{ display: 'flex', gap: 12, marginTop: 16 }}>
          <a href="#/" style={{ color: '#0969da', fontWeight: 600 }}>
            Repositories
          </a>
          <a href="#/chat" style={{ color: '#0969da', fontWeight: 600 }}>
            Chat
          </a>
          <a href="#/ask" style={{ color: '#0969da', fontWeight: 600 }}>
            Ask
          </a>
          <a href="#/search" style={{ color: '#0969da', fontWeight: 600 }}>
            Search
          </a>
          <a href="#/agents" style={{ color: '#0969da', fontWeight: 600 }}>
            Agents
          </a>
          <a href="#/auth" style={{ color: '#0969da', fontWeight: 600 }}>
            Auth
          </a>
          <a href="#/settings" style={{ color: '#0969da', fontWeight: 600 }}>
            Settings
          </a>
        </nav>
      </header>

      {route.kind === 'repo' ? (
        <RepoDetailPage
          repoId={route.repoId}
          initialPath={route.initialPath}
          initialTreePath={route.initialTreePath}
          initialRevision={route.initialRevision}
          from={route.from}
          searchHash={route.searchHash}
        />
      ) : null}
      {route.kind === 'auth' ? (
        <AuthPage key={hash} inviteId={route.inviteId} inviteEmail={route.inviteEmail} oauthCallback={route.oauthCallback} />
      ) : null}
      {route.kind === 'chat' ? <ChatPage initialRepoId={route.initialRepoId} initialThreadId={route.initialThreadId} /> : null}
      {route.kind === 'ask' ? <AskPage initialRepoId={route.initialRepoId} initialThreadId={route.initialThreadId} /> : null}
      {route.kind === 'search' ? <SearchPage key={hash} initialQuery={route.initialQuery} initialRepoId={route.initialRepoId} /> : null}
      {route.kind === 'agents' ? <AgentsPage initialRunId={route.initialRunId} /> : null}
      {route.kind === 'settings-landing' ? (
        <SettingsShell>
          <SettingsLandingPage />
        </SettingsShell>
      ) : null}
      {route.kind === 'settings-section' ? (
        <SettingsShell activeSection={route.section}>
          {route.section === 'connections' ? <SettingsConnectionsPage /> : null}
          {route.section === 'api-keys' ? <SettingsApiKeysPage /> : null}
          {route.section === 'members' ? <SettingsMembersPage /> : null}
          {route.section === 'access' ? <SettingsAccessPage /> : null}
          {route.section === 'linked-accounts' ? <SettingsLinkedAccountsPage /> : null}
          {route.section === 'oauth-clients' ? <SettingsOAuthClientsPage /> : null}
          {route.section === 'observability' ? <SettingsObservabilityPage /> : null}
          {route.section === 'review-automation' ? <SettingsReviewAutomationPage /> : null}
        </SettingsShell>
      ) : null}
      {route.kind === 'home' ? <RepoListPage /> : null}
    </main>
  );
}
